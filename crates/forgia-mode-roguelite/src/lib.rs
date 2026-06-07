//! # forgia-mode-roguelite
//!
//! 3e jeu Forgia V2 — roguelite FPS coop 1-3 joueurs (cible Steam Next Fest).
//! Story-468 (plan global) / Story-470 (M1 fondations).
//!
//! ## Scope M1 (cette release)
//!
//! - `RunState` SubStates de `GameMode::Roguelite` : Lobby / InRun / Boss / Defeat / Victory
//! - `StartRunEvent` / `EndRunEvent` (Bevy 0.18 `Message` derive)
//! - `RunSeed` Resource déterministe (xoshiro256**)
//! - Sensor `forgia2_roguelite_state.json` 1Hz
//!
//! Combat / loot / biome / coop / méta-progression : M2+ (voir story-468).
//!
//! ## Cleanup OnExit
//!
//! `RogueliteRunMarker` Component est exposé. Le système `sys_cleanup_run_markers`
//! qui despawne ces entités est géré par un **terminal parallèle dédié** — ce crate
//! ne contient PAS la logique de despawn pour éviter conflit merge.

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod atmosphere;
pub mod audio;
pub mod boons_apply;
pub mod coffre_sensor;
pub mod decor;
pub mod enemies;
pub mod hud;
pub mod intro_dialogue;
pub mod kill_popup;
pub mod loot_room;
pub mod poi;
pub mod run;
pub mod sensor;
pub mod shockwave;
pub mod stations;
pub mod toon_config;
pub mod waves;

pub use enemies::{EnemyArchetype, EnemyStats};
pub use waves::RogueliteWave;

pub use run::{EndRunEvent, RogueliteRunMarker, RunResult, RunSeed, RunState, StartRunEvent};
pub use sensor::RogueliteTelemetry;

pub mod prelude {
    pub use crate::{
        EndRunEvent, ForgiaModeRoguelitePlugin, RogueliteRunMarker, RunResult, RunSeed, RunState,
        StartRunEvent,
    };
}

pub struct ForgiaModeRoguelitePlugin;

impl Plugin for ForgiaModeRoguelitePlugin {
    fn build(&self, app: &mut App) {
        // M2 step 3 — Souls Resource + Pickup collection systems.
        if !app.is_plugin_added::<forgia_rpg_data::loot_tables::ForgiaLootTablesPlugin>() {
            app.add_plugins(forgia_rpg_data::loot_tables::ForgiaLootTablesPlugin);
        }
        // Story-558 Phase 3 (2026-05-29) — wire ForgiaBoonsPlugin
        // (Resources CoffreSession + ActiveBoons + BoonsCatalogue + events +
        // sys_handle_open_coffre + sys_handle_coffre_pick + asset loader).
        // Trigger OpenCoffreRequest sur transition into break vit dans
        // waves::sys_wave_orchestrator.
        if !app.is_plugin_added::<forgia_rpg_data::boons::ForgiaBoonsPlugin>() {
            app.add_plugins(forgia_rpg_data::boons::ForgiaBoonsPlugin);
        }
        // Story-558 Phase 4 — Boons apply : recompute PlayerCombatMods +
        // observer heal_on_kill.
        app.init_resource::<boons_apply::HealOnKillCumul>();
        app.add_systems(
            Update,
            (
                boons_apply::sys_recompute_boon_mods,
                boons_apply::sys_sync_player_health_guard,
                // Phase 4b — knockback + chain consomment CombatHitEvent.
                boons_apply::sys_apply_knockback_on_hit,
                boons_apply::sys_apply_chain_targets,
            )
                .chain()
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            OnExit(GameMode::Roguelite),
            (
                boons_apply::sys_reset_boon_mods,
                boons_apply::sys_remove_player_health_guard,
            ),
        );
        app.add_observer(boons_apply::obs_heal_on_kill);
        // Story-558 Phase 6 — sensor forgia2_coffre.json 1Hz
        app.init_resource::<coffre_sensor::CoffreSensorState>();
        app.add_systems(
            OnEnter(GameMode::Roguelite),
            coffre_sensor::sys_reset_coffre_sensor_on_run_start,
        );
        app.add_systems(
            Update,
            (
                coffre_sensor::sys_track_coffre_picks.in_set(GameSet::Effects),
                coffre_sensor::sys_write_coffre_sensor.in_set(GameSet::Sensors),
            ),
        );
        // V7 M3 step 2 — node-driven run loop (StageGraph Slay-the-Spire ratios).
        if !app.is_plugin_added::<forgia_stage::graph::ForgiaStageGraphPlugin>() {
            app.add_plugins(forgia_stage::graph::ForgiaStageGraphPlugin);
        }
        // Story-483 V7 P1 — data-driven stage arena (terrain + ramparts + POI anchors).
        if !app.is_plugin_added::<forgia_stage::ForgiaStageArenaPlugin>() {
            app.add_plugins(forgia_stage::ForgiaStageArenaPlugin);
        }
        // Debug collision (2026-06-05) — affiche TOUS les colliders physiques en
        // fil-de-fer (toggle F10) pour diagnostiquer les traversées de murs :
        // collider absent / désaligné vs mur visuel. Feature `debug-render-3d` ON.
        if !app.is_plugin_added::<bevy_rapier3d::render::RapierDebugRenderPlugin>() {
            app.add_plugins(bevy_rapier3d::render::RapierDebugRenderPlugin {
                enabled: false,
                ..default()
            });
        }
        app.add_systems(Update, sys_toggle_collider_debug);
        // Story-544 close (2026-05-29) — toon cel-shading + Sobel outline pour
        // direction cartoon bible v1. Genome-driven via roguelite_toon.toml
        // (hot-reload mtime 1Hz). Attaché OnEnter Roguelite, retiré OnExit.
        if !app.is_plugin_added::<forgia_postprocess::toon::ForgiaPpToonPlugin>() {
            app.add_plugins(forgia_postprocess::toon::ForgiaPpToonPlugin);
        }
        // OUTLINE — désactivé 2026-05-29 (root cause crash : wgpu panic
        // "SurfaceAcquireSemaphores still in use by SurfaceTexture"). Toon et
        // Outline déclarent les mêmes node_edges (Tonemapping → X → EndMainPass)
        // → render graph crée deux passes parallèles sur la même surface
        // texture. Fix futur : modifier OutlineSettings::node_edges pour insérer
        // APRÈS ToonSettings::node_label() au lieu de Tonemapping.
        // if !app.is_plugin_added::<forgia_postprocess::outline::ForgiaPpOutlinePlugin>() {
        //     app.add_plugins(forgia_postprocess::outline::ForgiaPpOutlinePlugin);
        // }
        app.add_systems(Startup, toon_config::sys_init_toon_genome);
        app.add_systems(
            OnEnter(GameMode::Roguelite),
            toon_config::sys_force_apply_toon_settings,
        );
        app.add_systems(
            Update,
            (
                toon_config::sys_hot_reload_toon_genome,
                toon_config::sys_apply_toon_settings,
            )
                .chain()
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            OnExit(GameMode::Roguelite),
            toon_config::sys_detach_toon_from_cameras,
        );
        app.add_systems(
            Update,
            toon_config::sys_write_toon_sensor.in_set(GameSet::Sensors),
        );
        // Story-561 (2026-06-03) — POI gameplay (loot vault + lava hazard + forge)
        // greffé sur les anchors AnchorKind::PoiSlot de forgia-stage. Genome
        // roguelite_poi_gameplay.toml hot-reload mtime. Sensor forgia2_stage_poi.json.
        app.init_resource::<poi::PoiStats>();
        app.add_systems(Startup, poi::sys_init_poi_genome);
        app.add_systems(OnEnter(GameMode::Roguelite), poi::sys_reset_poi_stats);
        app.add_systems(
            Update,
            (
                poi::sys_hot_reload_poi_genome,
                // Reset POI au départ de run (couvre le retry in-place REFORGER /
                // Nouvelle Run où OnEnter ne tire pas). Ordonné APRÈS sys_start_run
                // (RunSeed frais visible) et AVANT reconcile (despawn+clear flushés
                // avant re-spawn, même frame via AutoInsertApplyDeferred Bevy 0.18).
                poi::sys_clear_poi_on_run_start.after(run::sys_start_run),
                poi::sys_reconcile_poi_anchors.after(poi::sys_clear_poi_on_run_start),
            )
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            poi::sys_loot_vault_walkover
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            poi::sys_lava_tick
                .in_set(GameSet::Combat)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            poi::sys_write_poi_sensor.in_set(GameSet::Sensors),
        );
        // Story-562b (2026-06-03) — props décoratifs procéduraux pour remplir
        // l'arène (rochers/cristaux/piliers en anneau + débris au sol). Crate
        // libre, zéro touche forgia-stage. Genome roguelite_decor.toml hot-reload.
        app.add_systems(
            Startup,
            (decor::sys_init_decor_genome, decor::sys_load_decor_assets),
        );
        app.add_systems(
            Update,
            (
                decor::sys_hot_reload_decor_genome,
                decor::sys_reconcile_decor,
                decor::sys_calibrate_decor,
                decor::sys_decor_build_hull_colliders,
                decor::sys_unstick_bots_from_decor,
            )
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            decor::sys_write_decor_sensor.in_set(GameSet::Sensors),
        );
        // 2026-06-03 — pièces d'or qui tournent (CoinSpin) + aimantation des
        // pickups vers le joueur pendant l'écran de choix (Coffre) : plus besoin
        // de la caméra pour récupérer l'or restant (Hadès/Vampire Survivors).
        app.add_systems(
            Update,
            run::sys_magnetize_pickups_on_break
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_spin_coins
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Wisps d'âme (Âmes méta) : collecte walk-over → MetaSouls + anim flottante.
        app.add_systems(
            Update,
            (run::sys_collect_soul_wisps, run::sys_animate_soul_wisps)
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Caméra : clic HORS du panel de choix → look (override curseur/block_look).
        app.add_systems(
            Update,
            hud::sys_break_look_override
                .in_set(GameSet::UI)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Observer drop pickup on enemy death (filtré par EnemyArchetype).
        app.add_observer(run::obs_roguelite_enemy_death);
        // V7 M3 step 4 — Defeat trigger sur Player HP=0 (DeathEvent target==Player).
        app.add_observer(run::obs_roguelite_player_death);
        // Reset RogueliteWave OnEnter (relance run propre depuis lobby).
        app.add_systems(OnEnter(GameMode::Roguelite), reset_wave_resource);
        // 2026-05-21 — Auto-fire StartRunEvent OnEnter pour activer RunState
        // transitions (InRun) → débloque HUD wave/souls/defeat overlays gatés
        // run_state. Sans ça, l'utilisateur entre Roguelite, voit des bots mais
        // pas d'UI car sys_start_run n'est jamais déclenché.
        app.add_systems(OnEnter(GameMode::Roguelite), auto_start_run_on_enter);
        // Chrono de run — tick pendant InRun/Boss (pause-safe).
        app.add_systems(
            Update,
            run::sys_tick_run_timer
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Story-572 — sort F « Onde de choc » : input+dégâts (Combat), cooldown (Combat),
        // anim VFX (Effects). Tout gaté Roguelite.
        app.add_systems(
            Update,
            (shockwave::sys_shockwave_input, shockwave::sys_tick_shockwave_cooldown)
                .in_set(GameSet::Combat)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            (
                shockwave::sys_animate_shockwave_vfx,
                shockwave::sys_apply_knockback,
            )
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // V7 M2.5 — Tag PickupCollector en Update (PAS OnEnter) car Player spawn
        // par autre plugin (forgia-player::OnEnter AppMode::InGame), ordre cross-plugin
        // non garanti. Guard idempotent via `Without<PickupCollector>` (no-op après tag).
        app.add_systems(
            Update,
            run::sys_tag_player_as_collector
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );

        app.init_resource::<sensor::RogueliteTelemetry>()
            .init_resource::<waves::RogueliteWave>()
            // Story-558 Phase 5 — résumé Defeat (Or perdu / Souls conservées).
            .init_resource::<run::LastDefeatSummary>()
            // Story-571 — monnaie MÉTA persistante (distincte de l'Or in-run).
            .init_resource::<run::MetaSouls>()
            // Chrono de run (affiché sous la minimap).
            .init_resource::<run::RunTimer>()
            // Story-572 — sort F « Onde de choc » (AOE).
            .init_resource::<shockwave::ShockwaveAbility>()
            .add_sub_state::<RunState>()
            .add_message::<StartRunEvent>()
            .add_message::<EndRunEvent>()
            // P3 — telegraph boss enrage (UI banner + camera shake punch).
            .add_message::<waves::BossEnrageTriggeredEvent>()
            .add_systems(OnEnter(GameMode::Roguelite), run::sys_spawn_roguelite_scene)
            // Story-483 V7 P2 — Stage dispatch sur transition RunState
            // (Lobby/InRun/Boss). Insère StageLoadRequest avec stage_id dérivé.
            .add_systems(
                Update,
                run::sys_stage_dispatch
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Story-483 V7 P3 — Toggles emission (music_state / weather_override)
            // sur stage Ready. Émet RequestMusicState vers forgia-audio-music-state.
            .add_systems(
                Update,
                run::sys_apply_stage_toggles
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Story-483 V7 P1 — cleanup stage-arena entities + anchor stats on exit.
            .add_systems(
                OnExit(GameMode::Roguelite),
                forgia_stage::cleanup_stage_arena,
            )
            .add_systems(
                Update,
                (run::sys_start_run, run::sys_end_run)
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                Update,
                (
                    waves::sys_wave_orchestrator,
                    waves::sys_boss_enrage,
                    // TODO(story-471..479): sys_unstick_bots supprimé de crate::waves — re-implémenter
                    // waves::sys_unstick_bots,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite))
                    // Gèle la progression de vague pendant le parcours (loot room).
                    .run_if(loot_room::combat_running),
            )
            // V7 M3 step 3 — Health + Ammo stations walk-over collect (Effects set).
            .add_systems(
                Update,
                (
                    stations::sys_use_health_stations,
                    stations::sys_use_ammo_stations,
                    stations::sys_reset_stations_on_stage_change,
                )
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_plugins(hud::RogueliteHudPlugin)
            // Dialogue d'arrivée (bulle BD cartoon) — immersion bible v1 à l'entrée.
            .add_plugins(intro_dialogue::IntroDialoguePlugin)
            // Story-559 slice A — audio Roguelite (SFX impact/kill/hurt + ding
            // Or/Âmes + musique combat/break). Orthogonal : 0 édition cross-crate.
            .add_plugins(audio::RogueliteAudioPlugin)
            // Incrément 4 — atmosphère volcanique (brume DistanceFog + ambiante chaude).
            .add_plugins(atmosphere::RogueliteAtmospherePlugin)
            // Portail → salle de loot verticale (2026-06-06).
            .add_plugins(loot_room::RogueliteLootRoomPlugin)
            // Story-558 P2 Vlambeer juice — kill popup cartoon par archetype.
            .add_plugins(kill_popup::RogueliteKillPopupPlugin)
            // Sensor cross-mode : tourne en tout état (menu = run_state "none").
            // Telemetry tick counter en First pour capturer chaque frame.
            .add_systems(First, sensor::sys_update_roguelite_telemetry)
            .add_systems(
                Update,
                sensor::sys_write_roguelite_state.in_set(GameSet::Sensors),
            );
        // Cleanup OnExit(GameMode::Roguelite) géré par terminal parallèle (V7 cleanup
        // orchestration). Ne PAS dupliquer ici.
    }
}

/// Debug : toggle l'affichage fil-de-fer des colliders physiques (F10). Permet
/// de voir si les colliders (murs ramparts, salles, props) sont présents et
/// alignés avec les meshes visuels — diagnostic des traversées de murs.
fn sys_toggle_collider_debug(
    keys: Res<ButtonInput<KeyCode>>,
    mut ctx: ResMut<bevy_rapier3d::render::DebugRenderContext>,
) {
    if keys.just_pressed(KeyCode::F10) {
        ctx.enabled = !ctx.enabled;
        info!("[roguelite] Rapier collider debug render = {}", ctx.enabled);
    }
}

fn reset_wave_resource(mut wave: ResMut<waves::RogueliteWave>) {
    *wave = waves::RogueliteWave::default();
}

/// Marqueur sur les pickups pièce d'or (GLB Coin) — fait tourner la pièce.
#[derive(Component)]
pub struct CoinSpin;

/// Fait tourner les pièces d'or sur elles-mêmes (lecture "pièce").
fn sys_spin_coins(time: Res<Time>, mut q: Query<&mut Transform, With<CoinSpin>>) {
    let dr = time.delta_secs() * 2.6;
    for mut t in &mut q {
        t.rotate_y(dr);
    }
}

/// 2026-05-21 — Auto-fire StartRunEvent OnEnter(GameMode::Roguelite).
///
/// Sans ça, `sys_start_run` ne se déclenche jamais → `RunState` reste à
/// `Lobby` (default SubState) → HUD wave/souls/defeat (gated sur `RunState::InRun`)
/// reste invisible. Pattern Hadès "die-restart-die" : nouvelle entrée mode = nouveau run.
fn auto_start_run_on_enter(mut events: MessageWriter<run::StartRunEvent>) {
    events.write(run::StartRunEvent { seed: None });
    info!("[roguelite] auto_start_run_on_enter — StartRunEvent fired");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaModeRoguelitePlugin;
    }
}
