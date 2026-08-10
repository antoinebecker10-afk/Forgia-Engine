//! # Menu video background — fond vidéo du menu de sélection (porté de V1)
//!
//! Architecture : **streaming circular cache**, inspirée des sprites animés
//! Hades / Hollow Knight. **Aucun décodeur runtime ffmpeg** (zéro dette dep) —
//! juste des textures Bevy classiques affichées via egui.
//!
//! Pipeline :
//!   1. ffmpeg pré-extrait → `assets/ui/menu_video/frame_NNNN.webp` (24fps, 1280×720 q80)
//!   2. [`setup_menu_video`] : compte les fichiers, init [`MenuVideoState`], pré-warm
//!   3. [`menu_video_tick`] : avance `current_frame` à `MENU_VIDEO_FPS` (gated `AppMode::Menu`)
//!   4. [`ensure_menu_video_frame`] (appelé depuis `main_menu_ui`) : charge le
//!      lookahead, enregistre les `egui::TextureId`, drop hors fenêtre, renvoie
//!      le `TextureId` à peindre cette frame.
//!
//! VRAM peak = `lookahead × 1280×720 RGBA` ≈ 24 MB @ lookahead=8.
//!
//! Fallback gracieux : si `frame_count == 0` (dossier absent/vide) ou preroll
//! pas terminé, le menu retombe sur son fond sombre uni (aucun crash, aucun noir).
//!
//! Origine : porté du V1 `forgia-game/src/ui/start_menu.rs` (story-menu-video
//! 2026-05-03), validé runtime en V1. Adapté V2 : state `AppMode::Menu`, params
//! en const (pas de FpsTuning V2), sensor `forgia2_menu_video.json`.

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::AppMode;
use std::collections::{HashMap, HashSet};

/// Chemin du dossier des frames vidéo menu (relatif à `assets/`).
const MENU_VIDEO_DIR: &str = "ui/menu_video";

/// Frame count fallback — synchronisé avec l'extraction ffmpeg au build time.
/// Utilisé quand `std::fs::read_dir` échoue (cwd ≠ racine projet en build
/// distribué). Pattern industry standard : sprite atlas count baké dans le code.
/// **À mettre à jour si la vidéo source change** (`Forgia Menu .mp4` = 361 frames).
const MENU_VIDEO_FRAME_COUNT_FALLBACK: usize = 361;

/// FPS de lecture = fps d'extraction de la source (invariant de l'asset, pas un
/// paramètre gameplay → exception layout/asset de `no-hardcode.md`).
const MENU_VIDEO_FPS: f32 = 24.0;

/// Taille de la fenêtre LRU de frames gardées en VRAM (budget perf/VRAM).
const MENU_VIDEO_LOOKAHEAD: usize = 8;

/// Nombre minimum de frames `Loaded` simultanées avant de lancer le cycle.
/// Pattern Hades / Hollow Knight "preroll then play" : évite le noir/glitch au
/// cold launch tant que le décodeur WebP n'a pas pris assez d'avance.
const MENU_VIDEO_PREROLL_MIN_LOADED: usize = 4;

/// Cadence d'export du sensor JSON (secondes, `Time<Real>`).
const MENU_VIDEO_SENSOR_INTERVAL_S: f32 = 2.0;
/// Hors menu, la vidéo est inactive : garder un heartbeat peu fréquent évite des
/// écritures disque synchrones inutiles dans le hot path du jeu.
const MENU_VIDEO_INACTIVE_SENSOR_INTERVAL_S: f32 = 30.0;

/// Sensor : état du player vidéo menu.
const SENSOR_MENU_VIDEO_PATH: &str = "forgia2_menu_video.json";

/// Construit le path d'une frame (1-indexed pour matcher la convention ffmpeg `%04d`).
fn menu_video_frame_path(idx_1based: usize) -> String {
    format!("{MENU_VIDEO_DIR}/frame_{idx_1based:04}.webp")
}

/// State du player vidéo menu (Resource).
///
/// Cycle continu : `current_frame ∈ [1, frame_count]` avance à [`MENU_VIDEO_FPS`].
/// Cache LRU : seules les frames de la fenêtre `[current..current+lookahead]`
/// sont en VRAM. Les autres sont droppées (Bevy reclaim auto le `Handle<Image>`).
#[derive(Resource, Default)]
pub struct MenuVideoState {
    /// Nombre total de frames détectées. 0 = vidéo désactivée (fallback uni).
    pub frame_count: usize,
    /// Frame actuellement affichée (1-based pour matcher ffmpeg).
    pub current_frame: usize,
    /// Accumulateur de temps depuis le dernier swap (secondes).
    pub time_accumulator: f32,
    /// Cache : `frame_idx → (Handle<Image>, egui::TextureId)`.
    pub cache: HashMap<usize, (Handle<Image>, egui::TextureId)>,
    /// Latch : `true` une fois assez de frames livrées pour démarrer. Reste
    /// `true` ensuite (les retours menu→jeu→menu n'ont plus à preroll).
    pub prerolled: bool,
    /// Diagnostic : nb de frames de la fenêtre actuellement `Loaded`.
    pub frames_loaded: usize,
    /// Throttle de l'export sensor (secondes, `Time<Real>`).
    pub sensor_timer: f32,
    /// Dernier état exporté. Une transition Menu ↔ jeu doit être visible sans
    /// attendre le heartbeat inactif.
    pub last_sensor_menu_active: Option<bool>,
}

/// Setup (Startup) : compte les frames disponibles, init le state, pré-warm.
///
/// Stratégie robuste cwd-independent :
///   1. `std::fs::read_dir("assets/ui/menu_video")` (cwd-relatif, marche `cargo run`)
///   2. sinon `CARGO_MANIFEST_DIR/assets/...` (binaires hors workspace)
///   3. sinon fallback [`MENU_VIDEO_FRAME_COUNT_FALLBACK`] (build distribué)
pub fn setup_menu_video(mut commands: Commands, asset_server: Res<AssetServer>) {
    let primary_dir = std::path::Path::new("assets").join(MENU_VIDEO_DIR);
    let secondary_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(MENU_VIDEO_DIR);

    let count_webp = |dir: &std::path::Path| -> Option<usize> {
        std::fs::read_dir(dir).ok().map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|s| s.eq_ignore_ascii_case("webp"))
                })
                .count()
        })
    };

    let (frame_count, source) = match count_webp(&primary_dir) {
        Some(n) if n > 0 => (n, format!("scan {}", primary_dir.display())),
        _ => match count_webp(&secondary_dir) {
            Some(n) if n > 0 => (n, format!("scan {}", secondary_dir.display())),
            _ => (
                MENU_VIDEO_FRAME_COUNT_FALLBACK,
                "compile-time fallback".to_string(),
            ),
        },
    };

    info!("[menu-video] {frame_count} frames ({source})");

    // Sensor one-shot (observability-required §1) — via la file async comme
    // tous les capteurs (story-691 : c'était le dernier `std::fs::write`
    // synchrone du hub, une I/O bloquante sur le thread de jeu).
    let _ = forgia_core::sensor_io::enqueue(
        SENSOR_MENU_VIDEO_PATH,
        build_menu_video_sensor_json(MenuVideoSensorView {
            ts: 0.0,
            frame_count,
            current_frame: 0,
            cache_size: 0,
            frames_loaded: 0,
            prerolled: false,
            menu_active: false,
            time_accumulator: 0.0,
        }),
    );

    // Pré-warm cold-boot : kicke les loads dès le Startup pour saturer le
    // TaskPool IO et atteindre le preroll avant que l'user voie le menu.
    // Handles droppés volontairement — l'AssetServer retient le download.
    let pre_warm = (MENU_VIDEO_PREROLL_MIN_LOADED + MENU_VIDEO_LOOKAHEAD).min(frame_count);
    for i in 1..=pre_warm {
        let _: Handle<Image> = asset_server.load(menu_video_frame_path(i));
    }

    commands.insert_resource(MenuVideoState {
        frame_count,
        current_frame: 1,
        ..Default::default()
    });
}

/// Avance `current_frame` au rythme [`MENU_VIDEO_FPS`] quand on est dans le menu
/// et que le preroll est terminé. Utilise `Time<Real>` (wall-clock) : robuste
/// même si `Time<Virtual>` est pausé (leçon V1, bug "vidéo figée frame 73").
///
/// GELÉ quand le diorama d'arène couvre l'écran (story-691) : avancer la frame
/// force `ensure_menu_video_frame` à décoder ~24 WebP 1280×720/s et à uploader
/// leurs textures — pour une image que personne ne voit sous le fond opaque.
/// Figer la frame fige la fenêtre du cache : les 8 frames en VRAM restent
/// valides et `prerolled` reste vrai, donc couper `ui_backdrop_enabled` à chaud
/// réaffiche la vidéo immédiatement, sans re-preroll.
pub fn menu_video_tick(
    time: Res<Time<bevy::time::Real>>,
    app_state: Res<State<AppMode>>,
    covered: Res<crate::MenuBackdropCovered>,
    video: Option<ResMut<MenuVideoState>>,
) {
    if *app_state.get() != AppMode::Menu {
        return;
    }
    if covered.0 {
        return;
    }
    let Some(mut video) = video else { return };
    if video.frame_count == 0 || !video.prerolled {
        return;
    }
    let frame_period = 1.0 / MENU_VIDEO_FPS.max(1.0);
    video.time_accumulator += time.delta_secs();
    let frame_count = video.frame_count;
    // Skip-on-spike : avance plusieurs frames si on a accumulé plus d'une période
    // (ex: lag spike 200ms → saute 4 frames @ 24fps plutôt que de prendre du retard).
    while video.time_accumulator >= frame_period {
        video.time_accumulator -= frame_period;
        video.current_frame = (video.current_frame % frame_count) + 1;
    }
}

/// Gère le cache LRU + preroll et renvoie le `egui::TextureId` de la frame
/// courante à peindre (ou `None` → le menu peint son fond uni de fallback).
///
/// Appelé depuis `main_menu_ui` (qui détient `EguiContexts` dans le pass egui).
pub fn ensure_menu_video_frame(
    contexts: &mut EguiContexts,
    video: &mut MenuVideoState,
    asset_server: &AssetServer,
) -> Option<egui::TextureId> {
    if video.frame_count == 0 {
        return None;
    }
    let frame_count = video.frame_count;
    let current = video.current_frame.max(1);

    // Fenêtre désirée : {current, current+1, ..., current+lookahead-1} mod N.
    let desired: HashSet<usize> = (0..MENU_VIDEO_LOOKAHEAD)
        .map(|off| ((current - 1 + off) % frame_count) + 1)
        .collect();

    // Charge les frames manquantes (async, AssetServer.load) + enregistre TextureId.
    // `entry().or_insert_with` évite le double-lookup contains_key+insert (clippy).
    for &target in &desired {
        video.cache.entry(target).or_insert_with(|| {
            let handle: Handle<Image> = asset_server.load(menu_video_frame_path(target));
            let tex_id = contexts.add_image(bevy_egui::EguiTextureHandle::Strong(handle.clone()));
            (handle, tex_id)
        });
    }

    // LRU : désenregistrer D'ABORD les textures egui évincées. Retirer seulement
    // le Handle du HashMap ne suffit pas : `EguiContexts::add_image` conserve un
    // handle fort dans son registre, ce qui gardait les 361 frames WebP décodées
    // en RAM/VRAM après un cycle complet (~15 Go observés au menu).
    let evicted: Vec<AssetId<Image>> = video
        .cache
        .iter()
        .filter(|(idx, _)| !desired.contains(idx))
        .map(|(_, (handle, _))| handle.id())
        .collect();
    for image_id in evicted {
        contexts.remove_image(image_id);
    }
    // Les handles hors fenêtre peuvent alors réellement être libérés par Bevy.
    video.cache.retain(|k, _| desired.contains(k));

    // Preroll gate : compte les frames `Loaded`. Flip `prerolled` une seule fois.
    let loaded_now = video
        .cache
        .values()
        .filter(|(h, _)| matches!(asset_server.load_state(h.id()), LoadState::Loaded))
        .count();
    video.frames_loaded = loaded_now;
    if !video.prerolled && loaded_now >= MENU_VIDEO_PREROLL_MIN_LOADED {
        video.prerolled = true;
    }
    if !video.prerolled {
        return None;
    }

    // Frame courante prête ? Sinon fallback 1 tick (stall post-preroll rare).
    video.cache.get(&current).and_then(|(handle, tex_id)| {
        matches!(asset_server.load_state(handle.id()), LoadState::Loaded).then_some(*tex_id)
    })
}

/// Export périodique du sensor `forgia2_menu_video.json` (`Time<Real>`).
pub fn menu_video_sensor(
    time: Res<Time<bevy::time::Real>>,
    app_state: Res<State<AppMode>>,
    video: Option<ResMut<MenuVideoState>>,
) {
    let Some(mut video) = video else { return };
    let menu_active = *app_state.get() == AppMode::Menu;
    video.sensor_timer += time.delta_secs();
    let state_changed = video.last_sensor_menu_active != Some(menu_active);
    let interval = menu_video_sensor_interval(menu_active);
    if !state_changed && video.sensor_timer < interval {
        return;
    }
    video.sensor_timer = 0.0;
    video.last_sensor_menu_active = Some(menu_active);
    let json = build_menu_video_sensor_json(MenuVideoSensorView {
        ts: time.elapsed_secs(),
        frame_count: video.frame_count,
        current_frame: video.current_frame,
        cache_size: video.cache.len(),
        frames_loaded: video.frames_loaded,
        prerolled: video.prerolled,
        menu_active,
        time_accumulator: video.time_accumulator,
    });
    let _ = forgia_core::sensor_io::enqueue(SENSOR_MENU_VIDEO_PATH, json);
}

/// Vue sérialisable du sensor (testable sans World Bevy).
pub(crate) struct MenuVideoSensorView {
    pub ts: f32,
    pub frame_count: usize,
    pub current_frame: usize,
    pub cache_size: usize,
    pub frames_loaded: usize,
    pub prerolled: bool,
    pub menu_active: bool,
    pub time_accumulator: f32,
}

/// Construit le payload `forgia2_menu_video.json`. Pure : aucune I/O, aucun ECS.
pub(crate) fn build_menu_video_sensor_json(v: MenuVideoSensorView) -> String {
    format!(
        r#"{{"schema_version":1,"timestamp_secs":{ts:.1},"frame_count":{fc},"current_frame":{cf},"cache_size":{cs},"frames_loaded":{fl},"prerolled":{pr},"menu_active":{ma},"time_accumulator":{ta:.3}}}"#,
        ts = v.ts,
        fc = v.frame_count,
        cf = v.current_frame,
        cs = v.cache_size,
        fl = v.frames_loaded,
        pr = v.prerolled,
        ma = v.menu_active,
        ta = v.time_accumulator,
    )
}

fn menu_video_sensor_interval(menu_active: bool) -> f32 {
    if menu_active {
        MENU_VIDEO_SENSOR_INTERVAL_S
    } else {
        MENU_VIDEO_INACTIVE_SENSOR_INTERVAL_S
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_path_is_1indexed_4digit() {
        assert_eq!(menu_video_frame_path(1), "ui/menu_video/frame_0001.webp");
        assert_eq!(menu_video_frame_path(361), "ui/menu_video/frame_0361.webp");
    }

    #[test]
    fn sensor_json_well_formed() {
        let json = build_menu_video_sensor_json(MenuVideoSensorView {
            ts: 1.5,
            frame_count: 361,
            current_frame: 42,
            cache_size: 8,
            frames_loaded: 8,
            prerolled: true,
            menu_active: true,
            time_accumulator: 0.012,
        });
        assert!(json.contains("\"frame_count\":361"));
        assert!(json.contains("\"current_frame\":42"));
        assert!(json.contains("\"prerolled\":true"));
    }

    #[test]
    fn inactive_sensor_is_strongly_throttled() {
        assert_eq!(menu_video_sensor_interval(true), 2.0);
        assert_eq!(menu_video_sensor_interval(false), 30.0);
    }
}
