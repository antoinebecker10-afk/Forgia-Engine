//! Couche **sprite** — viewmodel pixel art rendu sur un quad, alternative au
//! `SceneRoot` GLB de [`crate::attach`].
//!
//! ## Pourquoi
//!
//! Les GLB viewmodel actuels sont des **maillages fusionnés** (`pepin.glb` :
//! 1 node, 1 mesh, 0 animation, 0 squelette). Aucune animation d'arme n'est
//! possible dessus : il n'y a ni chargeur, ni glissière, ni os à bouger. Un
//! sprite, lui, se compose de pièces indépendantes à la génération — le
//! rechargement devient une suite d'images.
//!
//! ## Ce que cette couche NE refait pas
//!
//! Tout le reste du pipeline continue de s'appliquer tel quel, parce que le quad
//! porte les mêmes composants que le viewmodel GLB :
//! [`WeaponViewmodel`] (donc `propagate_viewmodel_layer` lui met le layer 1),
//! un `Transform` piloté par [`crate::pose`] (sway, bob, ADS, recul) et
//! [`ViewmodelBaseScale`] (lerp de scale en ADS).
//!
//! ## Les durées ne sont pas ici
//!
//! Aucun clip ne porte sa propre durée. Le rechargement lit
//! [`ReloadState::progress`] — la même valeur que le gameplay — et le tir se cale
//! sur `fire_rate`. Écrire une durée d'animation à côté de `reload_time_secs`
//! serait la même grandeur écrite deux fois, et les deux divergeraient au premier
//! passage de balance : l'animation finirait avant que l'arme soit rechargée.

use bevy::asset::LoadState;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::GameMode;
use forgia_genome_core::Genome;
use forgia_player::prelude::FpsCamera;

use crate::attach::{ViewmodelBaseScale, WeaponViewmodel};
use crate::calibration::{viewmodel_target_size, viewmodel_transform};
use crate::genome::{
    lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeEntry, ViewmodelGenomeHandle,
};

/// Clip d'animation courant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpriteClip {
    #[default]
    Idle,
    Fire,
    Reload,
    /// Visée : l'arme vue plein dos. Clip distinct, pas un recadrage.
    Ads,
}

/// Viewmodel rendu en sprite. Porté par la même entité que [`WeaponViewmodel`].
#[derive(Component)]
pub struct SpriteViewmodel {
    pub idle: Vec<Handle<StandardMaterial>>,
    pub fire: Vec<Handle<StandardMaterial>>,
    pub reload: Vec<Handle<StandardMaterial>>,
    pub ads: Vec<Handle<StandardMaterial>>,
    /// Première image, gardée pour mesurer le cadre avant de bâtir le quad.
    pub probe: Handle<Image>,
    pub clip: SpriteClip,
    /// Temps écoulé dans le clip de tir (le rechargement, lui, n'a pas d'horloge :
    /// il lit la progression du gameplay).
    pub fire_elapsed: f32,
    /// Temps écoulé dans la boucle de repos — la seule qui ait son propre rythme.
    pub idle_elapsed: f32,
    /// Dernier chargeur observé — sa décrue déclenche le clip de tir. Évite de
    /// dépendre du bus d'events ammo pour une information déjà lisible d'état.
    pub last_mag: u32,
    /// Index réellement affiché, pour n'écrire le handle de matériau que sur
    /// changement de frame (sinon on invalide le binding à chaque frame).
    pub shown: Option<(SpriteClip, usize)>,
}

/// Marqueur : le quad attend que la première image soit chargée pour connaître
/// ses proportions. Même idiome que `NeedsAutoScale` côté GLB — on spawne masqué
/// et on révèle une fois mesuré.
#[derive(Component)]
pub struct NeedsSpriteMesh {
    pub target_size: f32,
}

/// `true` si l'arme courante est rendue en pixel art.
pub fn weapon_is_sprite(entry: Option<&ViewmodelGenomeEntry>) -> bool {
    entry.map(|e| !e.sprite_dir.is_empty()).unwrap_or(false)
}

/// Chemin d'une frame. Le radical vient du dernier segment du dossier
/// (`textures/weapons/pixel/pepin` → `pepin_reload_03.png`) : une convention
/// suffit, un champ de génome de plus serait une occasion de désynchronisation.
fn frame_path(dir: &str, clip: &str, index: usize) -> String {
    let stem = dir.rsplit('/').next().unwrap_or(dir);
    format!("{dir}/{stem}_{clip}_{index:02}.png")
}

fn load_clip(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    dir: &str,
    clip: &str,
    count: usize,
) -> (Vec<Handle<StandardMaterial>>, Option<Handle<Image>>) {
    let mut out = Vec::with_capacity(count);
    let mut first = None;
    for i in 0..count {
        let image: Handle<Image> = asset_server.load_with_settings(
            frame_path(dir, clip, i),
            // Échantillonnage au plus proche : sans ça, le filtrage bilinéaire
            // rend le pixel art flou, ce qui annule tout l'intérêt.
            |s: &mut ImageLoaderSettings| s.sampler = ImageSampler::nearest(),
        );
        if first.is_none() {
            first = Some(image.clone());
        }
        out.push(materials.add(StandardMaterial {
            base_color_texture: Some(image),
            // Non éclairé : un sprite porte déjà ses ombres peintes, une lumière
            // de scène par-dessus les contredirait.
            unlit: true,
            // Masque plutôt que fondu : bords nets et aucun tri de transparence.
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        }));
    }
    (out, first)
}

/// Spawne le viewmodel sprite si l'arme courante en déclare un et qu'aucun
/// viewmodel n'est encore attaché à la caméra.
pub fn spawn_sprite_viewmodel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_cam: Query<(Entity, Option<&Children>), With<FpsCamera>>,
    q_viewmodel: Query<&WeaponViewmodel>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    if !weapon_is_sprite(entry) {
        return;
    }
    let entry = entry.expect("weapon_is_sprite garantit Some");

    for (cam, children) in &q_cam {
        let has_vm = children
            .map(|c| c.iter().any(|child| q_viewmodel.get(child).is_ok()))
            .unwrap_or(false);
        if has_vm {
            continue;
        }

        let dir = entry.sprite_dir.as_str();
        let (idle, probe) = load_clip(
            &asset_server,
            &mut materials,
            dir,
            "idle",
            entry.sprite_idle_frames.max(1),
        );
        let (fire, _) = load_clip(
            &asset_server,
            &mut materials,
            dir,
            "fire",
            entry.sprite_fire_frames,
        );
        let (reload, _) = load_clip(
            &asset_server,
            &mut materials,
            dir,
            "reload",
            entry.sprite_reload_frames,
        );
        let (ads, _) = load_clip(
            &asset_server,
            &mut materials,
            dir,
            "ads",
            entry.sprite_ads_frames,
        );
        let Some(probe) = probe else {
            warn!("[forgia-viewmodel] sprite_dir '{dir}' sans frame idle — viewmodel ignoré");
            return;
        };

        let target = viewmodel_target_size(equipped.current, Some(entry));
        let mag = equipped.current_slot().map(|s| s.current_mag).unwrap_or(0);
        let vm = commands
            .spawn((
                WeaponViewmodel {
                    current: equipped.current,
                },
                SpriteViewmodel {
                    idle,
                    fire,
                    reload,
                    ads,
                    probe,
                    clip: SpriteClip::Idle,
                    fire_elapsed: 0.0,
                    idle_elapsed: 0.0,
                    last_mag: mag,
                    shown: None,
                },
                NeedsSpriteMesh {
                    target_size: target,
                },
                viewmodel_transform(equipped.current, Some(entry)),
                Visibility::Hidden,
                Name::new("WeaponViewmodel(sprite)"),
            ))
            .id();
        commands.entity(cam).add_child(vm);
        info!(
            "[forgia-viewmodel] viewmodel sprite spawné ({:?}, '{dir}', {} frames reload)",
            equipped.current, entry.sprite_reload_frames
        );
    }
}

/// Bâtit le quad une fois les proportions de l'image connues.
pub fn build_sprite_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    asset_server: Res<AssetServer>,
    q: Query<(Entity, &SpriteViewmodel, &NeedsSpriteMesh)>,
) {
    for (entity, sprite, needs) in &q {
        if matches!(
            asset_server.get_load_state(&sprite.probe),
            Some(LoadState::Failed(_))
        ) {
            warn!("[forgia-viewmodel] frame idle du sprite en échec — viewmodel retiré");
            commands.entity(entity).despawn();
            continue;
        }
        let Some(image) = images.get(&sprite.probe) else {
            continue;
        };
        let size = image.size();
        if size.y == 0 {
            continue;
        }
        // La hauteur porte `target_size` et la largeur suit les proportions de
        // l'image. Les proportions vivent dans le MESH, jamais dans `Transform.
        // scale` : la pose ADS y écrit un scale uniforme et les écraserait.
        let height = needs.target_size;
        let width = height * (size.x as f32 / size.y as f32);
        let mesh = meshes.add(Mesh::from(Rectangle::new(width, height)));

        let Some(material) = sprite.idle.first().cloned() else {
            continue;
        };
        commands
            .entity(entity)
            .remove::<NeedsSpriteMesh>()
            .insert((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                ViewmodelBaseScale(1.0),
                Visibility::Inherited,
            ));
        info!(
            "[forgia-viewmodel] quad sprite bâti {}×{} px → {:.2}×{:.2} m",
            size.x, size.y, width, height
        );
    }
}

/// Choisit la frame à afficher. Pur — testable sans `App`.
///
/// `reload_progress` vient de `ReloadState::progress` : l'animation est une
/// FONCTION de l'état gameplay, pas une horloge parallèle. Elle ne peut donc pas
/// dériver, et changer `reload_time_secs` la rythme automatiquement.
pub fn pick_frame(
    reloading: bool,
    reload_progress: f32,
    reload_frames: usize,
    firing: bool,
    fire_progress: f32,
    fire_frames: usize,
    idle_progress: f32,
    idle_frames: usize,
    aiming: bool,
    ads_frames: usize,
) -> (SpriteClip, usize) {
    if reloading && reload_frames > 0 {
        let i = (reload_progress.clamp(0.0, 1.0) * reload_frames as f32) as usize;
        return (SpriteClip::Reload, i.min(reload_frames - 1));
    }
    if firing && fire_frames > 0 {
        let i = (fire_progress.clamp(0.0, 1.0) * fire_frames as f32) as usize;
        return (SpriteClip::Fire, i.min(fire_frames - 1));
    }
    // La visée l'emporte sur le repos : c'est une autre vue de l'arme, pas un
    // état de moindre priorité. Elle boucle aussi — l'arme continue de cligner.
    if aiming && ads_frames > 0 {
        let i = (idle_progress.rem_euclid(1.0) * ads_frames as f32) as usize;
        return (SpriteClip::Ads, i.min(ads_frames - 1));
    }
    // Le repos BOUCLE quand l'arme en a les frames : Pépin est vivant, il cligne
    // des yeux. Une arme figée sur une image unique le trahirait.
    if idle_frames > 1 {
        let i = (idle_progress.rem_euclid(1.0) * idle_frames as f32) as usize;
        return (SpriteClip::Idle, i.min(idle_frames - 1));
    }
    (SpriteClip::Idle, 0)
}

/// Fait avancer l'animation et échange le matériau quand la frame change.
pub fn drive_sprite_animation(
    time: Res<Time>,
    ads: Res<crate::pose::AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<(&mut SpriteViewmodel, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let Some(entry) = entry else { return };
    let slot = equipped.current_slot();

    // Durée du clip de tir = le cycle de l'arme. Une arme qui tire 6 fois par
    // seconde ne peut pas jouer une animation de tir plus longue que 1/6 s.
    let fire_clip_secs = if entry.fire_rate > 0.0 {
        1.0 / entry.fire_rate
    } else {
        0.0
    };

    for (mut sprite, mut material) in &mut q {
        let mag = slot.map(|s| s.current_mag).unwrap_or(sprite.last_mag);
        if mag < sprite.last_mag {
            sprite.fire_elapsed = 0.0;
        }
        sprite.last_mag = mag;

        let firing = sprite.fire_elapsed < fire_clip_secs;
        if firing {
            sprite.fire_elapsed += time.delta_secs();
        }
        sprite.idle_elapsed += time.delta_secs();

        let reloading = slot.map(|s| s.reload_state.is_reloading()).unwrap_or(false);
        let reload_progress = slot
            .map(|s| s.reload_state.progress(s.config.reload_time_secs))
            .unwrap_or(0.0);
        let fire_progress = if fire_clip_secs > 0.0 {
            sprite.fire_elapsed / fire_clip_secs
        } else {
            0.0
        };

        let idle_progress = if entry.sprite_idle_secs > 0.0 {
            sprite.idle_elapsed / entry.sprite_idle_secs
        } else {
            0.0
        };
        let (clip, index) = pick_frame(
            reloading,
            reload_progress,
            sprite.reload.len(),
            firing,
            fire_progress,
            sprite.fire.len(),
            idle_progress,
            sprite.idle.len(),
            ads.progress > 0.5,
            sprite.ads.len(),
        );

        if sprite.shown == Some((clip, index)) {
            continue;
        }
        let frames = match clip {
            SpriteClip::Idle => &sprite.idle,
            SpriteClip::Fire => &sprite.fire,
            SpriteClip::Reload => &sprite.reload,
            SpriteClip::Ads => &sprite.ads,
        };
        if let Some(handle) = frames.get(index).cloned() {
            material.0 = handle;
            sprite.clip = clip;
            sprite.shown = Some((clip, index));
        }
    }
}

/// Despawn le viewmodel quand on passe d'une arme sprite à une arme GLB (ou
/// l'inverse) : les deux chemins de spawn respawneront le bon la frame suivante.
/// Sans ça, `update_viewmodel_on_switch` (qui exige `SceneRoot`) ignore
/// silencieusement l'entité sprite et le pistolet reste affiché sur un fusil.
pub fn despawn_on_render_kind_change(
    mut commands: Commands,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    q: Query<(Entity, Option<&SpriteViewmodel>), With<WeaponViewmodel>>,
) {
    if !equipped.is_changed() {
        return;
    }
    let wants_sprite = weapon_is_sprite(
        genome_handle
            .as_deref()
            .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current)),
    );
    for (entity, sprite) in &q {
        if sprite.is_some() != wants_sprite {
            commands.entity(entity).despawn();
        }
    }
}

/// Capteur `forgia2_viewmodel_sprite.json` (1 Hz).
///
/// Sans lui, un dossier de frames absent du dist donne un viewmodel invisible
/// sans le moindre diagnostic — exactement le mode d'échec que le capteur des
/// bras a été écrit pour couvrir.
pub fn write_sprite_sensor(
    time: Res<Time>,
    mut acc: Local<f32>,
    asset_server: Res<AssetServer>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    q: Query<(&SpriteViewmodel, Option<&NeedsSpriteMesh>)>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;

    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let declared = weapon_is_sprite(entry);
    let spawned = q.iter().count();
    let (clip, frame, pending, probe_state) = match q.iter().next() {
        Some((sprite, needs)) => {
            let state = match asset_server.get_load_state(&sprite.probe) {
                Some(LoadState::Loaded) => "loaded",
                Some(LoadState::Loading) => "loading",
                Some(LoadState::Failed(_)) => "failed",
                Some(LoadState::NotLoaded) => "not_loaded",
                None => "unknown",
            };
            let (c, f) = match sprite.shown {
                Some((SpriteClip::Reload, i)) => ("reload", i),
                Some((SpriteClip::Fire, i)) => ("fire", i),
                Some((SpriteClip::Idle, i)) => ("idle", i),
                Some((SpriteClip::Ads, i)) => ("ads", i),
                None => ("none", 0),
            };
            (c, f, needs.is_some(), state)
        }
        None => ("none", 0, false, "absent"),
    };

    let (severity, next_step) = severity_for_sprite(declared, spawned, probe_state, pending);
    let json = format!(
        r#"{{"id":"viewmodel_sprite","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"declared":{declared},"spawned":{spawned},"clip":"{clip}","frame":{frame},"awaiting_mesh":{pending},"probe_state":"{probe_state}"}}"#,
        time.elapsed_secs(),
    );
    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_viewmodel_sprite.json", json) {
        warn!("[forgia-viewmodel] sprite sensor write failed: {e}");
    }
}

/// Pur — testable sans `App`. Un capteur qui ne peut pas rougir ne mesure rien :
/// chacune de ces branches correspond à une panne réellement atteignable.
pub fn severity_for_sprite(
    declared: bool,
    spawned: usize,
    probe_state: &str,
    awaiting_mesh: bool,
) -> (&'static str, &'static str) {
    if declared && probe_state == "failed" {
        return (
            "critical",
            "frames pixel introuvables — vérifier assets/textures/weapons/pixel/<arme>/ (présent dans le dist ?)",
        );
    }
    if declared && spawned == 0 {
        return (
            "warn",
            "arme déclarée en sprite mais aucun quad spawné — FpsCamera présente ? voir spawn_sprite_viewmodel",
        );
    }
    if declared && awaiting_mesh {
        return (
            "info",
            "quad en attente de la première image — normal au boot, anormal si ça dure",
        );
    }
    ("ok", "")
}

/// Plugin sprite : spawn, construction du quad, animation, capteur.
pub struct ForgiaViewmodelSpritePlugin;

impl Plugin for ForgiaViewmodelSpritePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                despawn_on_render_kind_change,
                spawn_sprite_viewmodel,
                build_sprite_mesh,
                drive_sprite_animation,
                write_sprite_sensor,
            )
                .chain()
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_frame_follows_gameplay_progress() {
        // Le premier tiers du rechargement montre le premier tiers des frames :
        // l'animation ne peut pas finir avant l'arme, ni l'inverse.
        assert_eq!(pick_frame(true, 0.0, 14, false, 0.0, 3, 0.0, 1, false, 0).1, 0);
        assert_eq!(pick_frame(true, 0.5, 14, false, 0.0, 3, 0.0, 1, false, 0).1, 7);
        // Butée : progress == 1.0 ne doit pas sortir du tableau.
        assert_eq!(pick_frame(true, 1.0, 14, false, 0.0, 3, 0.0, 1, false, 0).1, 13);
    }

    #[test]
    fn reload_wins_over_fire() {
        let (clip, _) = pick_frame(true, 0.2, 14, true, 0.5, 3, 0.0, 1, false, 0);
        assert_eq!(clip, SpriteClip::Reload);
    }

    #[test]
    fn clip_vide_retombe_sur_idle() {
        // Une arme sans frames de tir ne doit pas indexer un tableau vide.
        assert_eq!(pick_frame(false, 0.0, 0, true, 0.5, 0, 0.0, 1, false, 0).0, SpriteClip::Idle);
    }

    #[test]
    fn frame_path_derive_le_radical_du_dossier() {
        assert_eq!(
            frame_path("textures/weapons/pixel/pepin", "reload", 3),
            "textures/weapons/pixel/pepin/pepin_reload_03.png"
        );
    }

    #[test]
    fn le_repos_boucle_quand_il_a_plusieurs_frames() {
        // Une arme vivante ne reste pas figée sur une image unique.
        assert_eq!(pick_frame(false, 0.0, 12, false, 0.0, 3, 0.0, 12, false, 0).1, 0);
        assert_eq!(pick_frame(false, 0.0, 12, false, 0.0, 3, 0.5, 12, false, 0).1, 6);
        // La boucle repart : 1.25 == 0.25 de tour.
        assert_eq!(pick_frame(false, 0.0, 12, false, 0.0, 3, 1.25, 12, false, 0).1, 3);
    }

    #[test]
    fn capteur_rouge_quand_les_frames_manquent() {
        let (sev, step) = severity_for_sprite(true, 0, "failed", false);
        assert_eq!(sev, "critical");
        assert!(!step.is_empty());
    }

    #[test]
    fn capteur_vert_quand_larme_est_en_glb() {
        // Non déclarée en sprite : aucune de ces branches ne doit s'allumer.
        assert_eq!(severity_for_sprite(false, 0, "absent", false).0, "ok");
    }
}
