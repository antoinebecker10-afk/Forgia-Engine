//! forgia-ai-arena-bot — Simple FPS Arena bot.
//!
//! State machine : Idle -> Chase (player in detect range) -> Attack (in attack range).
//! Shoots periodically at the BotTarget via rapier raycast + DamageEvent.
//! Listens for DeathEvent from forgia-damage to despawn + schedule respawn.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::GameMode;
use forgia_damage::{DamageEvent, DamageKind, DeathEvent, ForgiaDamagePlugin, Health, Mortal};

pub mod gizmos;
pub mod navpath;
pub mod tactical;
pub use navpath::BotPath;
pub use tactical::{BotAiSensor, TacticalTuning};

#[derive(Component, Debug, Clone, Copy)]
pub struct ArenaBot {
    pub state: BotState,
    pub speed: f32,
    pub detect_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    /// Cooldown restant avant prochain tir. Init = warmup (anti spawn-kill).
    pub attack_left: f32,
    /// Story-456 Phase 1 — distance d'arrêt en Chase (m). Bot s'arrête à `stop_distance`
    /// pour tirer au lieu de coller le player. Doit être < attack_range pour que le bot
    /// approche puis stagne en zone de tir. AAA standard 4-8m.
    pub stop_distance: f32,
    // ── Story-456 Phase 2 — LOS check ─────────────────────────────
    /// Ligne de vue claire vers player ? Recheck via `bot_los_check` à `los_check_hz`.
    pub has_los: bool,
    /// Timer décrémenté ; recheck LOS quand <= 0.
    pub los_check_left: f32,
    /// Grace window après acquisition LOS avant 1er tir (AAA reaction time ~350ms).
    pub los_grace_left: f32,
    // ── Story-456 Phase 3 — Strafing ──────────────────────────────
    /// Phase oscillation strafe sin (offset par-bot pour désync entre bots).
    pub strafe_phase_rad: f32,
    /// Random noise seed accumulé (xorshift32) pour bias strafe.
    pub strafe_noise_seed: u32,
    // ── Story-456 Phase 4 — Perception alert ──────────────────────
    /// True si player a tiré récemment dans rayon perception.
    pub alerted: bool,
    /// Timer décompte alerted (transition Idle→Chase forced pendant cette durée).
    pub alert_left: f32,
    /// Temps passé EN POURSUITE sans progresser (s). Voir `unstick_step`.
    pub stuck_secs: f32,
    /// Temps restant à longer l'obstacle au lieu de foncer vers la cible (s).
    pub unstick_left: f32,
    // ── Story-464 — LOS state gating ──────────────────────────────
    /// Temps restant autorisé à Chase post-perte de LOS (sec). Set par
    /// `bot_los_check` à la transition true→false. Décrémenté chaque frame.
    /// Tant que > 0 : Chase autorisé (last sight memory). Sinon Chase → Idle.
    pub los_lost_grace_left: f32,
    /// Story-685 — distance des PIEDS au centre du Transform (m).
    ///
    /// Le `Transform` d'un bot est le centre de sa capsule, pas ses pieds : il
    /// spawne à `capsule_half_height + capsule_radius + 0.05`. Snapper ce centre
    /// sur l'altitude du sol l'enterrerait de ~90 cm. Le suivi de sol pose donc
    /// `y = sol + foot_offset_m`.
    pub foot_offset_m: f32,
    /// Rayon d'emprise au sol (m) — la dimension HORIZONTALE, jumelle de
    /// `foot_offset_m`.
    ///
    /// # Pourquoi ce champ existe (2026-08-13)
    ///
    /// Il n'existait pas : la validation contre les murs utilisait une constante
    /// unique `BOT_BODY_RADIUS_M = 0.4`, commentée « valeur moyenne, marge
    /// conservatrice ». Elle n'est conservatrice pour personne :
    ///
    /// | archétype | rayon réel | ce que le code croyait | écart |
    /// |---|---|---|---|
    /// | sniper | 0,30 m | 0,40 m | +0,10 (s'arrête trop tôt) |
    /// | runner | 0,32 m | 0,40 m | +0,08 (s'arrête trop tôt) |
    /// | tank   | 0,55 m | 0,40 m | **−0,15 (pénètre)** |
    /// | boss   | 1,40 m | 0,40 m | **−1,00 (traverse)** |
    ///
    /// Les gros entraient dans la géométrie, les petits s'arrêtaient net au lieu de
    /// longer le mur — leur test de glissement échouait sur une largeur qu'ils
    /// n'avaient pas. Deux symptômes opposés, une seule cause.
    ///
    /// Même discipline que `foot_offset_m` : **une seule source**, le génome.
    pub body_radius_m: f32,
    /// Temps restant à TRAVERSER les obstacles (s). > 0 ⇒ collision ignorée.
    ///
    /// Filet de dernier recours, demandé en jeu le 2026-08-13 : un mob bloqué en
    /// poursuite finit par franchir ce qui le bloque, plutôt que de rester planté
    /// à la vue du joueur. Trois gardes le tiennent (durée de blocage, poursuite
    /// active, durée bornée) — cf. `arena_bots.toml` [ai] `phase_after_secs`.
    pub phase_left: f32,
}

impl Default for ArenaBot {
    fn default() -> Self {
        Self {
            state: BotState::Idle,
            speed: 4.0,
            detect_range: 25.0,
            attack_range: 1.8,
            attack_cooldown: 1.0,
            attack_left: 0.0,
            stop_distance: 1.5,
            has_los: false,
            los_check_left: 0.0,
            los_grace_left: 0.0,
            strafe_phase_rad: 0.0,
            strafe_noise_seed: 0xDEADBEEF,
            alerted: false,
            alert_left: 0.0,
            // Story-464 — grace de spawn 2s : bot peut Chase juste après spawn
            // même avant le 1er raycast LOS. Sinon bots gèlent jusqu'à
            // `1/los_check_hz` (~125ms). Resync via TacticalTuning au spawn caller.
            los_lost_grace_left: 2.0,
            // Valeur plausible pour une capsule d'humanoïde ; les spawns qui
            // connaissent leur capsule la remplacent par la vraie.
            foot_offset_m: 0.9,
            // Reflet de la capsule d'`arena_bots.toml` (`body_radius`). Les spawns
            // qui connaissent leur capsule la remplacent par la vraie — et ils la
            // connaissent tous, cf. `EnemyStats::capsule_radius`.
            body_radius_m: 0.40,
            phase_left: 0.0,
            stuck_secs: 0.0,
            unstick_left: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotState {
    Idle,
    Chase,
    Attack,
    Dead,
}

/// Configuration tir genome-driven. Inseré par le caller (e.g. forgia-mode-fps-arena)
/// au spawn. Permet aux différents modes d'avoir des bots avec damage / spread différents.
#[derive(Component, Debug, Clone, Copy)]
pub struct BotShootConfig {
    pub damage: f32,
    pub range: f32,
    /// Spread random ±jitter_deg appliqué sur la direction du tir (radians stockés).
    pub jitter_rad: f32,
    /// Couleur HDR du tracer (R, G, B linéaire — emissive multiplier).
    pub tracer_emissive: LinearRgba,
    /// Hauteur Y locale "shoulder" du bot (origine du raycast).
    pub shoulder_y: f32,
    /// Hauteur Y locale "torso" du player (cible du raycast). Default 1.0m.
    pub target_torso_y: f32,
}

impl Default for BotShootConfig {
    fn default() -> Self {
        Self {
            damage: 12.0,
            range: 35.0,
            jitter_rad: 4.0_f32.to_radians(),
            tracer_emissive: LinearRgba::new(4.0, 1.5, 0.5, 1.0),
            shoulder_y: 1.4,
            target_torso_y: 1.0,
        }
    }
}

/// Marker — entity is targeted by bots (typically the player).
#[derive(Component)]
pub struct BotTarget;

/// Marker — tracer transient ; despawn après son lifetime.
#[derive(Component)]
pub struct BotTracer {
    pub life_remaining: f32,
}

// ─── Boule de feu ennemie (projectile visible, story-617) ───────────────────
// Vitesse modérée = visible + esquivable à distance (feel mage Gunfire). Mesh
// émissif (pas hanabi) → toujours visible en plein air (cf boucherie_rocket).
const FIREBALL_SPEED: f32 = 26.0; // m/s
const FIREBALL_RADIUS: f32 = 0.3; // taille de la sphère visible
const FIREBALL_LIFETIME: f32 = 3.0; // s avant despawn si rien touché
const FIREBALL_HIT_RADIUS: f32 = 0.9; // proximité joueur pour infliger les dégâts

/// Projectile boule de feu lancé par un bot vers le joueur (intégration manuelle,
/// dégâts à l'arrivée → esquivable). Remplace le hitscan instantané.
#[derive(Component)]
pub struct BotFireball {
    pub vel: Vec3,
    pub age: f32,
    pub damage: f32,
    pub source: Entity,
}

/// Mesh + matériau partagés de la boule de feu (créés 1× au Startup → 0 alloc/tir).
#[derive(Resource)]
pub struct BotFireballAssets {
    pub mesh: Handle<Mesh>,
    pub mat: Handle<StandardMaterial>,
}

/// Mesh (constant) + cache de matériaux du tracer, indexé par couleur émissive.
/// Le mesh est construit 1× au Startup ; les matériaux sont résolus à la volée et
/// mis en cache par teinte → 0 alloc par tir après le 1er de chaque couleur, et
/// les tirs d'un même archétype partagent mesh+matériau (batching auto Bevy).
#[derive(Resource)]
pub struct BotTracerAssets {
    pub mesh: Handle<Mesh>,
    pub mats: bevy::platform::collections::HashMap<[u32; 4], Handle<StandardMaterial>>,
}

/// Startup — construit le mesh sphère + matériau émissif orange de la boule de feu.
fn setup_bot_fireball_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(FIREBALL_RADIUS));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.45, 0.1),
        emissive: LinearRgba::rgb(6.0, 2.0, 0.4),
        unlit: true,
        ..default()
    });
    commands.insert_resource(BotFireballAssets { mesh, mat });

    // Tracer : mesh fin constant (0.01×0.01×1.0) réutilisé par tous les tirs ;
    // les matériaux sont résolus/cachés par couleur au 1er tir de la teinte.
    let tracer_mesh = meshes.add(Cuboid::new(TRACER_CORE_WIDTH, TRACER_CORE_WIDTH, 1.0));
    commands.insert_resource(BotTracerAssets {
        mesh: tracer_mesh,
        mats: bevy::platform::collections::HashMap::default(),
    });
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BotSpawnPoint {
    pub position: Vec3,
    pub respawn_delay: f32,
}

#[derive(Resource, Default)]
pub struct PendingRespawns {
    pub queue: Vec<(f32, Vec3)>,
}

/// Resource simple seed PRNG xorshift32 — anti-perfect-aim sans dep `rand`.
#[derive(Resource)]
pub struct BotShootRng(pub u32);

impl Default for BotShootRng {
    fn default() -> Self {
        Self(0xCAFEBABE)
    }
}

impl BotShootRng {
    /// Next u32 xorshift32. Mutate self.
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Float in [-0.5, 0.5].
    fn next_signed_unit(&mut self) -> f32 {
        let u = self.next_u32() as f32 / u32::MAX as f32;
        u - 0.5
    }
}

pub struct ForgiaAiArenaBotPlugin;

impl Plugin for ForgiaAiArenaBotPlugin {
    fn build(&self, app: &mut App) {
        // ForgiaDamagePlugin idempotent — emit DamageEvent → mutate Health → DeathEvent.
        if !app.is_plugin_added::<ForgiaDamagePlugin>() {
            app.add_plugins(ForgiaDamagePlugin);
        }
        app.init_resource::<PendingRespawns>()
            .init_resource::<BotShootRng>()
            .init_resource::<TacticalTuning>()
            .init_resource::<BotAiSensor>()
            .init_resource::<gizmos::BotGizmoMode>()
            .add_systems(Startup, setup_bot_fireball_assets)
            .add_systems(
                Update,
                (
                    // Phase 2 LOS check + Phase 4 perception alert run AVANT
                    // state_machine pour que la state transition voie has_los/alerted à jour.
                    tactical::bot_los_check,
                    tactical::bot_perception_alert,
                    bot_state_machine,
                    // Story-700 inc.3 — le chemin s'entretient AVANT le mouvement, qui
                    // ne fait que lire le point courant. Sans maillage, ces deux
                    // systèmes sont quasi gratuits et le mouvement retombe en ligne
                    // droite : le comportement d'avant est le repli, pas l'exception.
                    navpath::sys_attach_bot_path,
                    tactical::sys_attach_bot_trace,
                    navpath::sys_bot_navpath,
                    // Phase 3 tactical_movement run APRÈS state_machine pour override le
                    // mouvement basique chase forward avec strafe + obstacle avoidance.
                    tactical::bot_tactical_movement,
                    // Story-517 — separation post-movement empêche les bots de
                    // se traverser (kinematic body ne push pas via physique).
                    tactical::bot_separation,
                    bot_attack_cooldown,
                    bot_shoot_at_target,
                    bot_fireball_fly,
                    bot_tracer_lifetime,
                    // Story-466 — handle_bot_deaths migré vers Observer
                    // `on_bot_death` (cf .add_observer ci-dessous).
                    tick_respawns,
                    tactical::write_bot_ai_sensor,
                    tactical::write_bot_traces_sensor,
                )
                    .chain(),
            )
            // Gizmos de debug : coût NUL quand F10 est sur Off — un gizmo par bot
            // et par frame ne se paie pas pour rien.
            .add_systems(
                Update,
                gizmos::sys_bot_gizmos.run_if(|m: Res<gizmos::BotGizmoMode>| m.dessine_les_bots()),
            )
            .add_observer(on_bot_death);
    }
}

/// Story-456 Phase 1 — state machine 3-tier :
/// - dist <= stop_distance : Attack (à portée tir, ne bouge plus)
/// - stop_distance < dist <= detect_range : Chase (s'approche)
/// - dist > detect_range : Idle (hors perception)
///
/// Story-464 — gate Chase/Attack sur LOS récente OU alert audio. Sans ces deux
/// flags, le bot ne doit pas Chase à travers les murs même si distance OK.
/// Fonction pure exposée pour tests headless (cf `tests/los_gating.rs`).
pub fn decide_bot_state(bot: &ArenaBot, dist: f32) -> BotState {
    let has_recent_sight = bot.has_los || bot.los_lost_grace_left > 0.0;
    let can_pursue = has_recent_sight || bot.alerted;
    if dist <= bot.stop_distance && can_pursue {
        BotState::Attack
    } else if dist <= bot.detect_range && can_pursue {
        BotState::Chase
    } else {
        BotState::Idle
    }
}

fn bot_state_machine(
    mut bots: Query<(&mut ArenaBot, &mut Transform), Without<BotTarget>>,
    targets: Query<&Transform, With<BotTarget>>,
    time: Res<Time>,
) {
    let Some(target) = targets.iter().next() else {
        return;
    };
    let target_pos = target.translation;
    let dt = time.delta_secs();

    for (mut bot, mut xf) in &mut bots {
        if bot.state == BotState::Dead {
            continue;
        }
        let to_target = target_pos - xf.translation;
        let dist = to_target.length();

        // Story-456 Phase 1 — state machine 3-tier :
        // - dist <= stop_distance : Attack (à portée tir, ne bouge plus)
        // - stop_distance < dist <= detect_range : Chase (s'approche)
        // - dist > detect_range : Idle (hors perception)
        //
        // Story-464 — gate Chase/Attack sur LOS via `decide_bot_state` (testable).
        bot.state = decide_bot_state(&bot, dist);

        // Story-456 Phase 3 — mouvement Chase délégué à `tactical::bot_tactical_movement`
        // (strafe + obstacle avoidance). Ce système ne gère plus que la state transition
        // + l'orientation visuelle ci-dessous. Le `let _ = dt` évite l'unused warning.
        let _ = dt;

        // Toujours faire face au target (même statique) pour orientation tracer + visuel.
        if dist > 0.01 {
            let dir = to_target / dist;
            let yaw = (-dir.x).atan2(-dir.z);
            xf.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

fn bot_attack_cooldown(time: Res<Time>, mut bots: Query<&mut ArenaBot>) {
    let dt = time.delta_secs();
    for mut bot in &mut bots {
        bot.attack_left = (bot.attack_left - dt).max(0.0);
    }
}

/// Système : pour chaque bot in Attack/Chase avec cooldown ready, raycast vers
/// le player → si hit player → DamageEvent + spawn tracer visuel. Reset cooldown.
#[allow(clippy::too_many_arguments)]
fn bot_shoot_at_target(
    mut bots: Query<(Entity, &mut ArenaBot, &GlobalTransform, &BotShootConfig)>,
    targets: Query<&GlobalTransform, With<BotTarget>>,
    mut rng: ResMut<BotShootRng>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fireball_assets: Res<BotFireballAssets>,
    mut tracer_assets: ResMut<BotTracerAssets>,
) {
    let Some(target_tf) = targets.iter().next() else {
        return;
    };

    let target_pos = target_tf.translation();

    for (bot_entity, mut bot, bot_tf, config) in &mut bots {
        if bot.state == BotState::Dead || bot.attack_left > 0.0 {
            continue;
        }
        if !matches!(bot.state, BotState::Chase | BotState::Attack) {
            continue;
        }
        // Story-456 Phase 2 — gate tir sur LOS + grace window (AAA reaction time).
        // Sans LOS = pas de tir (player derrière mur). Pendant grace = vu mais "réfléchit".
        if !bot.has_los || bot.los_grace_left > 0.0 {
            continue;
        }

        // Origine raycast = position bot + shoulder_y
        let origin = bot_tf.translation() + Vec3::Y * config.shoulder_y;
        // Cible visée = torse player
        let aim_at = Vec3::new(
            target_pos.x,
            target_pos.y + config.target_torso_y,
            target_pos.z,
        );
        let to_target = aim_at - origin;
        let dist = to_target.length();
        if dist < 0.5 || dist > config.range {
            // Trop proche (overlap) ou hors range → skip shoot (mais cooldown pas reset)
            continue;
        }
        let base_dir = to_target / dist;

        // Jitter : random tilt sur 2 axes perpendiculaires.
        let right = base_dir.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(base_dir).normalize_or_zero();
        let jx = rng.next_signed_unit() * config.jitter_rad;
        let jy = rng.next_signed_unit() * config.jitter_rad;
        let shot_dir = (base_dir + right * jx + up * jy).normalize_or_zero();
        if shot_dir.length_squared() < 0.001 {
            continue;
        }

        // Boule de feu projectile (story-617) — remplace le hitscan instantané :
        // VISIBLE + esquivable. Les dégâts sont infligés à l'arrivée par
        // `bot_fireball_fly`. Aimée via `shot_dir` (LOS déjà garantie ci-dessus).
        commands.spawn((
            Mesh3d(fireball_assets.mesh.clone()),
            MeshMaterial3d(fireball_assets.mat.clone()),
            Transform::from_translation(origin),
            BotFireball {
                vel: shot_dir * FIREBALL_SPEED,
                age: 0.0,
                damage: config.damage,
                source: bot_entity,
            },
        ));

        // Flash de bouche court au tireur (feedback ; longueur fixe, plus de raycast).
        spawn_tracer(
            &mut commands,
            &mut tracer_assets,
            &mut materials,
            origin,
            shot_dir,
            4.0,
            config.tracer_emissive,
        );

        bot.attack_left = bot.attack_cooldown;
    }
}

/// Largeur du cœur du tracer (rendering interne, cf story-452). Constant.
const TRACER_CORE_WIDTH: f32 = 0.01;

fn spawn_tracer(
    commands: &mut Commands,
    tracer_assets: &mut ResMut<BotTracerAssets>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    dir: Vec3,
    length: f32,
    emissive: LinearRgba,
) {
    // Story-452 (2026-05-18 PM) — fix screenshot user : bot tracers étaient des laser
    // beams de 30m × 4cm bouchant l'écran. Solution : segment court (max 4m) au lieu
    // de full hit_dist, mesh fin (1cm), HDR /3, lifetime court.
    //
    // Perf (audit 2026-07-01) : mesh constant pré-warmé + matériau caché par couleur
    // → 0 alloc `meshes.add`/`materials.add` par tir après le 1er de chaque teinte.
    let key = [
        emissive.red.to_bits(),
        emissive.green.to_bits(),
        emissive.blue.to_bits(),
        emissive.alpha.to_bits(),
    ];
    let mat = tracer_assets
        .mats
        .entry(key)
        .or_insert_with(|| {
            let dimmed = LinearRgba::new(
                emissive.red * 0.35,
                emissive.green * 0.35,
                emissive.blue * 0.35,
                emissive.alpha,
            );
            materials.add(StandardMaterial {
                emissive: dimmed,
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })
        })
        .clone();
    // Tracer segment court (4m max) partant du canon, pas tout le ray.
    let seg_len = length.min(4.0);
    let mid = origin + dir * (seg_len * 0.5);
    let tf = Transform::from_translation(mid)
        .looking_to(dir, Vec3::Y)
        .with_scale(Vec3::new(1.0, 1.0, seg_len));
    commands.spawn((
        Mesh3d(tracer_assets.mesh.clone()),
        MeshMaterial3d(mat),
        tf,
        BotTracer {
            life_remaining: 0.08, // 80ms (était 120ms)
        },
    ));
}

fn bot_tracer_lifetime(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut BotTracer)>,
) {
    let dt = time.delta_secs();
    for (e, mut tracer) in &mut q {
        tracer.life_remaining -= dt;
        if tracer.life_remaining <= 0.0 {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}

/// Fait voler les boules de feu ennemies en ligne droite. Dégâts à l'ARRIVÉE
/// (proximité du joueur) → esquivable si on bouge. Despawn à l'impact ou en fin
/// de vie. (story-617). La boule vise où était le joueur au tir → bouger = esquive.
fn bot_fireball_fly(
    time: Res<Time>,
    mut commands: Commands,
    mut fireballs: Query<(Entity, &mut Transform, &mut BotFireball)>,
    targets: Query<(Entity, &GlobalTransform), With<BotTarget>>,
    mut damage_events: MessageWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    let target = targets.iter().next();
    for (e, mut tf, mut fb) in &mut fireballs {
        fb.age += dt;
        tf.translation += fb.vel * dt;

        let mut hit = false;
        if let Some((player_e, player_tf)) = target {
            // Torse joueur ≈ +1 m (cohérent avec `target_torso_y` du tir).
            let torso = player_tf.translation() + Vec3::Y;
            if tf.translation.distance(torso) <= FIREBALL_HIT_RADIUS {
                damage_events.write(DamageEvent {
                    target: player_e,
                    source: Some(fb.source),
                    amount: fb.damage,
                    kind: DamageKind::Physical,
                });
                hit = true;
            }
        }

        if hit || fb.age >= FIREBALL_LIFETIME {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}

/// Story-466 — handler de mort bot migré MessageReader → Observer (Bevy 0.18).
/// Trigger automatique via `commands.trigger(DeathEvent)` côté forgia-damage.
/// Évite le polling 1×/frame du `MessageReader`. Pattern recommandé pour les
/// events one-shot per-entity (cf bevy_ecs::event::EntityEvent docs 0.18).
///
/// 2026-05-29 — gate cross-mode via Option<Res<State<GameMode>>> (pattern
/// canonique memory [[reference_observer_cross_mode_gate_via_state_read]],
/// Bevy 0.18 Observer ne supporte pas `.run_if()`). En Roguelite : despawn
/// l'ennemi mais ne queue PAS de respawn (chaque wave a un compte fixé par
/// `wave_composition`, respawn auto ferait fuiter le compteur `bots_alive`
/// → wave bloquée).
fn on_bot_death(
    event: On<DeathEvent>,
    mut commands: Commands,
    bots: Query<(&Transform, Option<&BotSpawnPoint>), With<ArenaBot>>,
    q_ascends: Query<(), With<forgia_effects::prelude::AscendsOnDeath>>,
    mut pending: ResMut<PendingRespawns>,
    game_mode: Option<Res<State<GameMode>>>,
) {
    let target = event.target;
    let Ok((xf, spawn)) = bots.get(target) else {
        return;
    };
    let is_roguelite = matches!(
        game_mode.as_deref().map(|s| s.get()),
        Some(GameMode::Roguelite)
    );
    if !is_roguelite {
        let pos = spawn.map(|s| s.position).unwrap_or(xf.translation);
        let delay = spawn.map(|s| s.respawn_delay).unwrap_or(3.0);
        pending.queue.push((delay, pos));
    }
    let _ = xf;
    // 2026-08-05 — un corps qui s'envole n'est balayé par PERSONNE : c'est
    // `forgia-effects::death_ascension` qui le despawne à la fin de l'envol.
    //
    // Ce garde est le jumeau de celui de `despawn_dead_cubes` (forgia-fps).
    // L'envol des morts a exempté ce balayeur-là et oublié celui-ci : le corps
    // disparaissait ici pendant le flush du `trigger(DeathEvent)`, avant même
    // d'avoir reçu `Ascending` — d'où le panic « Entity despawned » sur l'insert.
    // Deux balayeurs pour un seul concept : ils portent désormais le même garde.
    if q_ascends.contains(target) {
        return;
    }
    if let Ok(mut ec) = commands.get_entity(target) {
        ec.try_despawn();
    }
}

fn tick_respawns(
    time: Res<Time>,
    mut pending: ResMut<PendingRespawns>,
    mut commands: Commands,
    game_mode: Option<Res<State<GameMode>>>,
) {
    // 2026-05-29 — en Roguelite, drain la queue sans respawn. Garde-fou si des
    // respawns avaient été queuées avant entrée mode (race OnEnter vs ticks).
    if matches!(
        game_mode.as_deref().map(|s| s.get()),
        Some(GameMode::Roguelite)
    ) {
        pending.queue.clear();
        return;
    }
    // Perf (audit 2026-07-01) : la queue est vide la quasi-totalité des frames →
    // early-return AVANT l'alloc `Vec::new()` (le cas non-vide est rare = respawn).
    if pending.queue.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let mut ready = Vec::new();
    pending.queue.retain_mut(|(t, pos)| {
        *t -= dt;
        if *t <= 0.0 {
            ready.push(*pos);
            false
        } else {
            true
        }
    });
    for pos in ready {
        commands.spawn((
            ArenaBot::default(),
            Mortal,
            Health::new(60.0),
            Transform::from_translation(pos),
            GlobalTransform::default(),
            BotSpawnPoint {
                position: pos,
                respawn_delay: 3.0,
            },
            RigidBody::KinematicPositionBased,
            Collider::capsule_y(0.6, 0.4),
        ));
    }
}

pub mod prelude {
    pub use crate::{
        ArenaBot, BotShootConfig, BotShootRng, BotSpawnPoint, BotState, BotTarget, BotTracer,
        ForgiaAiArenaBotPlugin, PendingRespawns,
    };
}

#[cfg(test)]
mod death_sweep_tests {
    use super::*;
    use forgia_damage::DamageKind;
    use forgia_effects::prelude::AscendsOnDeath;

    /// Monte le strict nécessaire pour faire tourner l'observer de mort.
    fn app_with_observer() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<PendingRespawns>();
        app.add_observer(on_bot_death);
        app
    }

    fn kill(app: &mut App, target: Entity) {
        app.world_mut().trigger(DeathEvent {
            target,
            source: None,
            final_kind: DamageKind::Physical,
        });
        app.update();
    }

    /// L'invariant qui a coûté un crash : deux balayeurs de mort existent
    /// (`despawn_dead_cubes` et celui-ci) et un corps qui s'envole doit
    /// échapper aux DEUX. Le corriger d'un seul côté laissait cet observer
    /// despawner la cible pendant le flush, avant son `Ascending`.
    #[test]
    fn an_ascending_body_is_never_swept_by_the_death_observer() {
        let mut app = app_with_observer();
        let bot = app
            .world_mut()
            .spawn((
                ArenaBot::default(),
                Transform::default(),
                AscendsOnDeath::default(),
            ))
            .id();

        kill(&mut app, bot);

        assert!(
            app.world().get_entity(bot).is_ok(),
            "un corps qui s'envole doit survivre à l'observer — c'est \
             death_ascension qui le despawne à la fin de l'envol"
        );
    }

    /// Le pendant : sans le marqueur, le balayage reste celui d'avant.
    #[test]
    fn a_plain_bot_is_still_swept() {
        let mut app = app_with_observer();
        let bot = app
            .world_mut()
            .spawn((ArenaBot::default(), Transform::default()))
            .id();

        kill(&mut app, bot);

        assert!(
            app.world().get_entity(bot).is_err(),
            "un bot sans envol doit toujours être despawné par l'observer"
        );
    }
}
