//! waves.rs — M2 step 4 : système 3-wave end-to-end Roguelite.
//!
//! Pipeline :
//! 1. OnEnter(GameMode::Roguelite) : `RogueliteWave::default()` Resource inséré
//! 2. wave 1 spawn auto (caller : `sys_spawn_roguelite_scene` appelle `spawn_wave_enemies(1)`)
//! 3. `sys_wave_orchestrator` poll Query<&ArenaBot> chaque frame :
//!    - si bots_alive == 0 ET wave en cours : démarre break (3s)
//!    - quand break_secs_left <= 0 : spawn next wave (current_wave +=1)
//!    - quand current_wave >= WAVES_TOTAL : `boss_defeated=true` → la porte du
//!      socle s'ouvre (story-603 ; plus d'`EndRunEvent(Victory)` auto)
//!
//! Scaling : wave 1 = 8 ennemis (3T/3R/2S), wave 2 = 12 (4T/4R/4S), wave 3 = 16 (6T/6R/4S).

use crate::enemies::{self, EnemyArchetype};
use crate::run::RogueliteRunMarker;
use crate::wave_comp::WaveCompConfig;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody, Sensor};
use forgia_ai_arena_bot::ArenaBot;
use forgia_core::prelude::*;
use forgia_rpg_data::boons::OpenCoffreRequest;
// Story-490 — Health type swap forgia_damage → forgia_combat pour matcher la
// query `find_health_ancestor` de forgia-fps hitscan (qui scanne
// `Query<&mut forgia_combat::Health, With<TargetCube>>`). Sans ce swap, type
// mismatch silencieux → hits classifiés `BlockerNonZone` au lieu de damage.
// cf memory [[reference-dual-health-type-trap]] et [[reference-bevy-rapier-child-collider-pattern-2026-05-20]].
use crate::defense::DefenseConfig;
use crate::enemies::EnemyStatsConfig;
use forgia_combat::Health;
use forgia_damage::Mortal;
use forgia_mode_fps_arena::TargetCube;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;

// Story-646 R2 — l'ex-`WAVES_TOTAL: u8 = 3` (3 vagues dont boss dans UNE salle) est
// remplacé par la structure multi-salles : `RunGraphConfig.waves_per_stage` vagues
// par salle combat (gene `roguelite_waves_per_stage`) × `boss_depth` salles + salle Boss.
// Story-558 Phase 1 (2026-05-29) — break 3.0 → 15.0s.
// 15s = window prep ammo/heal + Coffre du Forgeron (Phase 3) + HP reset (AC10).
// Best practice industry (audit roguelite-engagement-2026-05-29 §1) : break
// court (3s) ne laisse pas le temps de respirer pour cible enfants/femmes ;
// 15s = sweet spot Hadès Chamber transition.
pub const BREAK_SECS: f32 = 15.0;
const WAVE_BASE_SEED: u64 = 0xC0FF_EE51_C0BA_1700;
/// Story-672 — nombre d'angles essayés sur l'anneau avant de retenir le moins
/// mauvais. 24 = un pas de 15°. Plomberie de placement (spawn ponctuel, pas un
/// hot path) : ce n'est pas un levier de gameplay.
const SPAWN_CLEAR_TRIES: u32 = 24;

#[derive(Resource, Debug, Clone)]
pub struct RogueliteWave {
    /// Story-646 R2 — salle courante (0-indexed). Salles 0..boss_depth = combat
    /// (`waves_per_stage` vagues chacune), salle boss_depth = Boss (1 vague boss).
    pub stage: u8,
    pub current_wave: u8,
    pub bots_alive: u32,
    pub break_secs_left: f32,
    /// True quand on est en train d'attendre le prochain spawn (entre 2 vagues).
    pub in_break: bool,
    /// 2026-08-04 — le combat est fini ET la prochaine étape est un CHANGEMENT
    /// D'ARÈNE : on attend que le joueur franchisse la porte.
    ///
    /// Le minuteur de break reste la fenêtre de prep entre deux combats d'une
    /// *même* arène. Changer de pièce, en revanche, ne se fait plus tout seul :
    /// « tant qu'on ne clique pas, ça n'enchaîne pas ». Lu par le HUD pour
    /// afficher le bouton, et par l'orchestrateur pour geler le minuteur.
    pub awaiting_room_entry: bool,
    /// L'arène courante est celle du BOSS.
    ///
    /// Nécessaire parce que `current_wave` y est **surchargé** : il porte
    /// `BOSS_WAVE_COMPOSITION` (une composition), pas un indice de combat. Sans
    /// ce drapeau, `round()` lisait ce 3 comme « 3ᵉ combat de l'arène » et
    /// annonçait le round **12** au lieu de 10 — donc une menace de ×9,29 au
    /// lieu de ×6,91. Le boss se battait 35 % trop dur, et le HUD n'en montrait
    /// rien (le compteur est clampé au total). Observé en jeu le 2026-08-04.
    pub is_boss_arena: bool,
    pub victory_emitted: bool,
    /// Story-603 — true dès que la vague finale (boss) est nettoyée. Ouvre la
    /// porte du socle (`loot_room::sys_reconcile_boss_gate`). Remplace l'ancienne
    /// émission `EndRunEvent(Victory)` (décision user 2026-06-17 : pas de victoire
    /// auto, boucle boss → porte → parcours → arène). Reset au start de run.
    pub boss_defeated: bool,
    /// Gate anti-race (ex-`Local` de l'orchestrateur, story-646) : il faut avoir VU
    /// ≥1 frame avec des bots vivants avant de pouvoir « clear » — resettable par
    /// salle (le `Local` ne l'était pas → instant-clear à l'entrée d'une salle).
    pub seen_alive: bool,
    /// Story-646 Inc.2 — portes proposées après le clear (vide = pas de choix en
    /// cours). Remplies par l'orchestrateur depuis `graph.stages[next]` ; l'overlay
    /// (`hud::draw_portal_overlay`) affiche + capte le choix.
    pub portal_choices: Vec<forgia_stage::graph::StageKind>,
    /// Index de la porte choisie (écrit par l'overlay, consommé par l'orchestrateur).
    pub portal_pick: Option<u8>,
    /// Kind de la salle courante (porte choisie ; None = salle 0 / boss / fallback).
    /// Story-669 — CONSOMMÉ par `wave_comp::compose` : c'est ce qui rend le choix
    /// de porte réel. Il était écrit puis lu nulle part (hors un `info!`).
    pub room_kind: Option<forgia_stage::graph::StageKind>,
    /// Story-669 — budget de difficulté du nœud de graph choisi
    /// (`StageNode.difficulty_budget`). Ce champ était calculé à chaque run par
    /// `director_budget_for_depth` puis JETÉ : zéro lecteur dans tout le workspace.
    /// Il pilote maintenant la densité d'ennemis de la salle. 0 = pas de graph.
    pub room_budget: u32,
}

impl Default for RogueliteWave {
    fn default() -> Self {
        Self {
            stage: 0,
            current_wave: 1,
            bots_alive: 0,
            break_secs_left: 0.0,
            in_break: false,
            awaiting_room_entry: false,
            is_boss_arena: false,
            victory_emitted: false,
            boss_defeated: false,
            seen_alive: false,
            portal_choices: Vec::new(),
            portal_pick: None,
            room_kind: None,
            room_budget: 0,
        }
    }
}

impl RogueliteWave {
    /// Le **ROUND** canonique, 1-indexé : le combat en cours depuis le début du
    /// chapitre, toutes arènes confondues.
    ///
    /// ## Pourquoi cette méthode existe (2026-08-04)
    ///
    /// `stage` compte les **arènes**, pas les rounds : plusieurs combats se
    /// déroulent dans la même arène (`waves_per_stage`). Les deux étaient
    /// confondus, donc le compteur du HUD, les paliers de difficulté et la
    /// montée de menace lisaient un index d'arène **en croyant lire un round**.
    /// Avec 3 combats par arène, la menace aurait monté par bonds de trois
    /// pendant que le compteur affichait « ROUND 1 / 10 » trois fois de suite.
    ///
    /// Elle est calculée, jamais stockée : deux nombres pour la même chose
    /// finissent toujours par diverger — c'est la leçon déjà écrite en tête de
    /// `hud::run_progress_label`.
    ///
    /// ```text
    /// waves_per_stage = 3
    /// arène 0 → rounds 1,2,3   arène 1 → 4,5,6   arène 2 → 7,8,9   arène 3 → 10 (boss)
    /// ```
    pub fn round(&self, waves_per_stage: u8) -> u32 {
        let wps = u32::from(waves_per_stage.max(1));
        // L'arène du boss ne contient qu'UN affrontement, et son `current_wave`
        // porte une COMPOSITION (`BOSS_WAVE_COMPOSITION`), pas un indice de
        // combat. Le lire comme un indice gonflait le round — et donc la menace
        // réellement appliquée aux ennemis.
        if self.is_boss_arena {
            return u32::from(self.stage) * wps + 1;
        }
        u32::from(self.stage) * wps + u32::from(self.current_wave)
    }
}

/// Vague « boss » (branche `_` de `wave_comp::compose`). La salle Boss (story-646)
/// spawn cette composition directement.
pub const BOSS_WAVE_COMPOSITION: u8 = 3;

/// Le joueur franchit la porte : on passe à l'arène suivante.
///
/// Écrit par le bouton du HUD (« ENTRER DANS LA PIÈCE SUIVANTE » / « AFFRONTER LE
/// BOSS »), consommé par `sys_wave_orchestrator`. C'est la seule chose qui fait
/// changer d'arène en boucle de rounds — il n'y a plus de minuteur pour ça.
#[derive(Message, Debug, Clone, Copy)]
pub struct EnterNextRoomRequest;

/// Ramène un point d'apparition DANS l'enceinte de l'arène.
///
/// ## Pourquoi c'est nécessaire (2026-08-04, rapporté en jeu)
///
/// Depuis story-686, l'anneau d'apparition est centré sur le **joueur** — le
/// correctif était juste : avant, il était centré sur l'origine et les bots
/// naissaient à l'autre bout de la carte. Mais rien ne borne l'anneau : collé aux
/// remparts, un rayon de 25 ou 50 m (runner, sniper) déborde **derrière le mur**,
/// et l'ennemi naît hors de l'arène. « Si je suis proche des murailles, les mobs
/// spawn en dehors de la map. »
///
/// On garde la DIRECTION voulue et on raccourcit le rayon : l'ennemi arrive
/// toujours d'où l'anneau le voulait, simplement plus près. Le ramener au centre
/// serait pire — tous les ennemis surgiraient dans le dos du joueur.
///
/// `arena_r <= 0` → aucune enceinte connue, on ne touche à rien.
///
/// PUR — testable sans App.
pub fn clamp_into_arena(p: Vec2, arena_r: f32, body_r: f32) -> Vec2 {
    if arena_r <= 0.0 {
        return p;
    }
    // ⚠️ `arena_extent_m` est le rayon du cercle CIRCONSCRIT : les remparts
    // hexagonaux y sont INSCRITS, donc leurs murs passent à 0,866 × R au plus
    // près (l'apothème). Borner au cercle laissait une couronne entre l'apothème
    // et le rayon où l'on naît DEHORS — « il y a encore des mobs qui spawn
    // derrière les remparts », rapporté après le premier correctif.
    //
    // Le ratio est DÉRIVÉ et déjà nommé dans `forgia_stage::layout`, on le lit
    // au lieu d'en écrire une seconde copie.
    let apotheme = arena_r * forgia_stage::layout::HEX_INSCRIBED_RATIO;
    let limite = (apotheme - body_r.max(0.0)).max(0.5);
    let d = p.length();
    if d <= limite || d <= f32::EPSILON {
        return p;
    }
    p * (limite / d)
}

/// Le gain d'Âmes après application des atouts de **récolte**.
///
/// Arrondi au plus proche, et **jamais moins que le gain nu** : un multiplicateur
/// dégénéré (0, négatif, NaN) ne doit pas pouvoir retirer au joueur ce qu'il
/// aurait touché sans atout. Un bonus ne peut être qu'un bonus.
///
/// PUR — testable sans App.
pub fn recolte(base: u32, mul: f32) -> u32 {
    if !mul.is_finite() || mul <= 1.0 {
        return base;
    }
    ((base as f32 * mul).round() as u32).max(base)
}

/// L'ARÈNE où se tient le boss, dérivée du nombre de ROUNDS d'un chapitre.
///
/// Les deux grandeurs ne sont pas dans la même unité : `max_rounds` compte des
/// combats, une arène en contient `waves_per_stage`. Le boss occupe sa propre
/// arène, juste après celles qui portent les rounds de combat.
///
/// ```text
/// max_rounds = 10, waves_per_stage = 3
///   rounds 1..9  → arènes 0,1,2   (⌈9/3⌉ = 3 arènes)
///   round  10    → arène 3         ← le boss
/// ```
///
/// Pur — testable sans App ni World.
pub fn boss_arena_for(max_rounds: u32, waves_per_stage: u8) -> u8 {
    let wps = u32::from(waves_per_stage.max(1));
    // `max_rounds - 1` = les rounds de COMBAT ; le boss est le dernier round et
    // n'entre pas dans ce compte. `max_rounds = 1` (chapitre réduit au boss) donne
    // donc 0 arène de combat et le boss en arène 0.
    let combat_rounds = max_rounds.saturating_sub(1);
    combat_rounds.div_ceil(wps).min(u32::from(u8::MAX)) as u8
}

/// Les 3 configs nécessaires au spawn d'une vague, en un seul `SystemParam`.
///
/// `sys_start_run` était DÉJÀ à 16 params — le plafond dur de Bevy. Ajouter la
/// config de composition l'aurait fait déborder. `scalability.md` prescrit
/// exactement ce remède : « SystemParam bundle quand > 12 params ».
#[derive(bevy::ecs::system::SystemParam)]
pub struct WaveSpawnConfigs<'w, 's> {
    pub stats: Res<'w, EnemyStatsConfig>,
    pub defense: Res<'w, DefenseConfig>,
    pub comp: Res<'w, WaveCompConfig>,
    /// Story-677 — la boucle de rounds. Dans le bundle et pas en param direct :
    /// `sys_wave_orchestrator` était à 14 params, le plafond Bevy est 16.
    pub rounds: Res<'w, crate::rounds::RoundsConfig>,
    /// Story-686 — la position du JOUEUR, qui centre l'anneau d'apparition.
    ///
    /// Dans le bundle et pas en param direct : `sys_start_run` est déjà AU
    /// plafond de 16. Et c'est sa place — l'anneau fait partie de ce qu'il faut
    /// pour poser une vague.
    pub player: Query<'w, 's, &'static Transform, With<forgia_player::Player>>,
    /// 2026-08-04 — atouts « récolte ». Même raison d'être ici que les autres :
    /// l'orchestrateur est au plafond de params, et le gain d'Âmes fait partie
    /// de ce qu'il faut pour clore une vague.
    pub mods: Res<'w, forgia_combat::combat_mods::PlayerCombatMods>,
    /// 2026-08-04 — l'enceinte réellement bâtie, pour que l'anneau d'apparition
    /// ne déborde pas derrière les remparts quand le joueur y est collé.
    pub stage_result: Option<Res<'w, forgia_stage::StageLoadResult>>,
}

impl WaveSpawnConfigs<'_, '_> {
    /// Centre de l'anneau d'apparition = position du joueur, en XZ.
    ///
    /// Repli sur l'origine si le joueur n'existe pas encore : c'est le
    /// comportement d'avant, et il vaut mieux qu'un `panic` au chargement.
    pub fn ring_center(&self) -> Vec2 {
        self.player
            .iter()
            .next()
            .map(|t| t.translation.xz())
            .unwrap_or(Vec2::ZERO)
    }

    /// Rayon de l'enceinte, LU sur ce que l'arène a réellement bâti.
    ///
    /// `StageLoadResult.extent_m` est publié par `forgia-stage` au moment où il
    /// pose l'enceinte : c'est la seule valeur qui ne peut pas diverger de la
    /// géométrie. Recopier un rayon depuis un génome en serait une seconde, et
    /// c'est exactement la classe de défaut qu'on passe la journée à supprimer.
    ///
    /// Absent (arène pas encore bâtie) → `0` = aucune borne, comportement d'avant.
    pub fn arena_radius(&self) -> f32 {
        self.stage_result.as_deref().map(|r| r.extent_m).unwrap_or(0.0)
    }

    /// Contexte de spawn pour une vague donnée.
    #[allow(clippy::too_many_arguments)]
    pub fn ctx<'a>(
        &'a self,
        wave: u8,
        stage: u8,
        kind: Option<forgia_stage::graph::StageKind>,
        density: f32,
        run_seed: u64,
        obstacles: &'a crate::decor::DecorObstacles,
    ) -> WaveSpawnCtx<'a> {
        WaveSpawnCtx {
            // Story-686 — lu ICI, une seule fois : les 3 sites d'appel ne
            // peuvent pas en donner une version différente.
            ring_center: self.ring_center(),
            arena_radius: self.arena_radius(),
            stats: &self.stats,
            defense: &self.defense,
            comp: &self.comp,
            wave,
            stage,
            kind,
            density,
            run_seed,
            obstacles,
        }
    }
}

/// Contexte de spawn d'une vague (story-669).
///
/// Bundle plutôt que 10 paramètres : `spawn_wave_enemies` reçoit désormais la
/// SALLE, son TYPE et la GRAINE DE RUN — les trois entrées qui manquaient à
/// l'ancienne `wave_composition(wave: u8)` et dont l'absence figeait la boucle
/// (mêmes ennemis, mêmes places, choix de porte sans effet).
pub struct WaveSpawnCtx<'a> {
    pub stats: &'a EnemyStatsConfig,
    pub defense: &'a DefenseConfig,
    pub comp: &'a WaveCompConfig,
    /// Vague dans la salle (1, 2, … ; `BOSS_WAVE_COMPOSITION` = boss).
    pub wave: u8,
    /// Profondeur de salle (0-indexée) — pour la graine et le log.
    pub stage: u8,
    /// Type de la salle choisie à la porte. `None` = graph absent → neutre.
    pub kind: Option<forgia_stage::graph::StageKind>,
    /// `budget_director(salle) / budget_director(0)`, borné par le genome.
    pub density: f32,
    /// Story-686 — CENTRE de l'anneau d'apparition : la position du JOUEUR.
    ///
    /// L'anneau était centré sur l'origine du monde. Ça n'a jamais été un choix :
    /// ça marchait parce que le joueur apparaissait AUSSI à l'origine, donc les
    /// deux coïncidaient. Story-682 a déplacé le spawn joueur de 12 m pour le
    /// sortir du puits de `forge_sanctum` — et la relation s'est cassée.
    ///
    /// Mesuré en jeu : un tank né de l'autre côté du puits se retrouvait à 24 m
    /// du joueur pour 22 m de portée de détection. Il ne l'acquérait jamais et
    /// restait Idle à vie. 8 bots vivants, 0 en poursuite, round jamais nettoyé.
    ///
    /// Le génome dit « le Tank arrive PRÈS, le Sniper LOIN » — près de QUOI, si
    /// ce n'est du joueur ? Le rayon a toujours voulu dire « distance au
    /// joueur » ; il était implémenté en « distance à l'origine ».
    pub ring_center: Vec2,
    /// Rayon de l'enceinte. `0` = inconnue → aucune borne appliquée.
    pub arena_radius: f32,
    /// Graine de la RUN. Sans elle, les positions étaient les mêmes à chaque run.
    pub run_seed: u64,
    /// Story-672 — emprises solides du décor de la salle. Un ennemi ne doit JAMAIS
    /// apparaître dedans : les bots n'ont pas de navmesh, ils poussent contre le
    /// collider indéfiniment au lieu de le contourner.
    pub obstacles: &'a crate::decor::DecorObstacles,
}

/// Spawn N ennemis de la composition wave donnée. Caller : OnEnter scene ou
/// orchestrator au passage de vague.
///
/// Story-517 (2026-05-26) : visual KayKit Skeleton SceneRoot + head proxy
/// sensor pour permettre headshots (zone-based damage multiplier via
/// `forgia_damage::HitZoneTag`). Capsule physique conservée (collision OK).
///
/// Story-652 (2026-07-02) : la sphère tête est dimensionnée sur le crâne MESURÉ du
/// GLB et DÉPASSE de la capsule (recalibrée aux épaules) — sinon le raycast
/// premier-hit de forgia-fps ne la touchait jamais (0 headshot possible). Elle est
/// ensuite recollée sur le joint `head` du rig animé (cf `head_hitbox.rs`).
/// Story-669 : la composition et le placement viennent désormais de
/// `wave_comp::compose` (genome + salle + type de salle) et la graine de spawn
/// dérive de la RUN, plus d'une constante.
pub fn spawn_wave_enemies(
    commands: &mut Commands,
    asset_server: &AssetServer,
    ctx: &WaveSpawnCtx<'_>,
) -> u32 {
    let stats_cfg = ctx.stats;
    let def_cfg = ctx.defense;
    let wave = ctx.wave;
    let composition = crate::wave_comp::compose(ctx.comp, wave, ctx.kind, ctx.density);
    // Graine de placement : RUN × salle × vague. Avant story-669 c'était
    // `WAVE_BASE_SEED ^ wave` — une CONSTANTE : le joueur mémorisait en 2 runs où
    // arrivent les 3 Tanks, et deux runs différentes étaient superposables.
    let place_seed = ctx
        .run_seed
        .rotate_left(u32::from(ctx.stage) % 64)
        ^ u64::from(wave).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ WAVE_BASE_SEED;
    let mut yaw_rng = Xoshiro256StarStar::seed_from_u64(place_seed);
    let jitter_m = ctx.comp.ring.jitter_m.max(0.0);
    let mut total = 0u32;
    for (archetype, count, ring_radius) in &composition {
        let stats = stats_cfg.for_archetype(*archetype);
        let skeleton_handle: Handle<Scene> = asset_server.load(
            // KayKit GLB scene root : `#Scene0` est la convention Bevy GltfLoader.
            format!("{}#Scene0", enemies::skeleton_asset_path(*archetype)),
        );
        let scene_scale = enemies::skeleton_scale(*archetype);
        // Story-652 — sphère tête = crâne mesuré du GLB (position bind-pose en
        // fallback ; recollée sur l'os `head` par sys_track_head_proxies ensuite).
        let head_y_offset = crate::head_hitbox::head_local_y_bind(
            stats.capsule_half_height,
            stats.capsule_radius,
            scene_scale,
        );
        let head_radius = crate::head_hitbox::head_radius(stats.head_radius, scene_scale);
        let yaw0 = (yaw_rng.next_u64() as f64 / u64::MAX as f64) as f32 * std::f32::consts::TAU;
        for i in 0..*count {
            let theta = yaw0 + (i as f32 / *count as f32) * std::f32::consts::TAU;
            // Dispersion du rayon (genome `ring.jitter_m`) : sans elle, les ennemis
            // se posent sur un anneau parfait, identique d'une salle à l'autre.
            // Le rayon reste positif quoi qu'il arrive (0.5 m plancher).
            let unit = (yaw_rng.next_u64() as f64 / u64::MAX as f64) as f32 * 2.0 - 1.0;
            let r = (ring_radius + unit * jitter_m).max(0.5);
            // Story-672 — l'angle voulu peut tomber dans un prop solide. On balaie
            // l'anneau pour trouver une place libre ; si tout est encombré on prend
            // le point le plus dégagé. Un ennemi apparaît toujours, jamais dedans.
            let theta = ctx.obstacles.clear_angle_on_ring_at(
                ctx.ring_center,
                r,
                theta,
                stats.capsule_radius,
                SPAWN_CLEAR_TRIES,
            );
            // Story-686 posait le centre sur le joueur ; ceci empêche l'anneau
            // de déborder l'enceinte quand il est collé à un rempart.
            let voulu = Vec2::new(
                ctx.ring_center.x + r * theta.cos(),
                ctx.ring_center.y + r * theta.sin(),
            );
            let pose = clamp_into_arena(voulu, ctx.arena_radius, stats.capsule_radius);
            let x = pose.x;
            let z = pose.y;
            // Story-685 — MÊME source que `foot_offset_m` de l'ArenaBot : le suivi
            // de sol repose sur l'égalité des deux, deux copies divergeraient.
            let y = stats.foot_offset_m();

            // Pattern miroir forgia-mode-fps-arena::wave::spawn_wave_bots:343 :
            // PARENT = Health + TargetCube + RigidBody + ArenaBot (PAS de Collider).
            // CHILD 1 = Collider body capsule (HitZone::Body default).
            // CHILD 2 = SceneRoot KayKit visual (skeleton GLB), rotation_y(PI) pour
            //           aligner KayKit +Z forward vs Bevy -Z forward
            //           (memory : reference_kaykit_skeleton_forward_axis_pi.md).
            // CHILD 3 = Head proxy sensor sphere (HitZone::Head), Y offset.
            let parent = commands
                .spawn((
                    Name::new(format!("RogueliteEnemy_W{wave}_{}_{i}", archetype.label())),
                    RogueliteRunMarker,
                    DespawnOnExit(GameMode::Roguelite),
                    *archetype,
                    TargetCube,
                    Transform::from_xyz(x, y, z),
                    RigidBody::KinematicPositionBased,
                    Health::new(stats.hp),
                    // Story-640 P0-2 — couche défensive (bouclier bleu / armure jaune)
                    // AU-DESSUS de la Vie. Le hit de base (forgia-fps) la draine avant
                    // `combat::Health` ; régén hors combat par `defense::sys_regen_defense`.
                    def_cfg.layer_for(*archetype),
                    Mortal,
                    // Story-640 P0-2 — bot config depuis la config LIVE (hot-reload).
                    stats_cfg.arena_bot(*archetype),
                    // Story-517 fix : ennemis n'avaient pas BotShootConfig → ne
                    // tiraient pas. Damage + range différencié par archetype.
                    stats_cfg.bot_shoot(*archetype),
                    // Story-636 — échantillon de vitesse pour le driver d'anim
                    // squelettique (marche vs course selon le déplacement réel).
                    crate::enemy_anim::EnemyLocoSample::default(),
                    // Story-644 (intent préservé par story-652) — nameplate au-dessus
                    // du CRÂNE réel (haut de la sphère tête + marge). L'ancien ancrage
                    // « haut de capsule + 0.6 » plaçait le nameplate dans le crâne du
                    // Runner et 2 m au-dessus du Boss (capsule décalibrée du mesh).
                    forgia_enemy_nameplate::NameplateAnchor(head_y_offset + head_radius + 0.35),
                ))
                .id();
            // Body collider (capsule), classified HitZone::Body par défaut.
            // Story-517 fix : Sensor → player KCC passe à travers (no contact force)
            // mais raycast hitscan le détecte toujours (QueryFilter::default n'exclut
            // pas les sensors). Permet au joueur de traverser les ennemis en combat
            // rapproché tout en gardant le hitscan body-zone fonctionnel.
            commands.spawn((
                Name::new(format!(
                    "RogueliteEnemy_W{wave}_{}_{i}_body",
                    archetype.label()
                )),
                ChildOf(parent),
                Transform::default(),
                Collider::capsule_y(stats.capsule_half_height, stats.capsule_radius),
                Sensor,
            ));
            // Chaque ennemi garde son rig KayKit complet : animation, rattachement
            // de la hitbox tête au joint `head` et silhouette cohérente. Le profil
            // Tracy 17 a identifié le sous-arbre `DemoLevel` comme source majeure
            // des pics actuels ; un proxy global par vague dégradait inutilement le
            // combat sans traiter cette racine statique.
            commands
                .spawn((
                    Name::new(format!(
                        "RogueliteEnemy_W{wave}_{}_{i}_visual",
                        archetype.label()
                    )),
                    ChildOf(parent),
                    SceneRoot(skeleton_handle.clone()),
                    // KayKit forward = +Z, Bevy parent yaw uses -Z → rotate PI.
                    // Y offset : KayKit pivot au sol → translate down par
                    // (capsule_half_height + capsule_radius) pour aligner les
                    // pieds avec le BAS de la capsule parent (sinon lévitation).
                    Transform::from_xyz(
                        0.0,
                        -(stats.capsule_half_height + stats.capsule_radius),
                        0.0,
                    )
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(scene_scale)),
                ))
                // Story-636 — au scene-ready : rend le mesh translucide (clone de
                // matériau dédupliqué) pour la viz de contrôle du rig.
                .observe(crate::enemy_rig_debug::on_enemy_scene_ready);
            // Head proxy sensor (story-652) : sphère HitZone::Head dépassant de la
            // capsule → atteignable en premier-hit. Suivie du joint `head` du rig
            // animé une fois la scène liée (head_hitbox::sys_bind_head_joints).
            commands.spawn((
                Name::new(format!(
                    "RogueliteEnemy_W{wave}_{}_{i}_head_proxy",
                    archetype.label()
                )),
                ChildOf(parent),
                Transform::from_xyz(0.0, head_y_offset, 0.0),
                Collider::ball(head_radius),
                Sensor,
                forgia_damage::HitZoneTag(forgia_damage::HitZone::Head),
                crate::head_hitbox::HeadProxy {
                    enemy_root: parent,
                    joint: None,
                },
            ));
            total += 1;
        }
    }
    info!("[roguelite] Wave {wave} spawned : {total} enemies");
    total
}

/// Tourne chaque frame en GameMode::Roguelite. Update bots_alive et orchestre
/// les transitions de vague.
///
/// 2026-05-29 — `seen_alive` Local<bool> gate anti-race :
/// `Commands.spawn()` est différé jusqu'au prochain ApplyDeferred. Si l'ordre
/// de schedule fait tourner orchestrator AVANT que les spawns de `sys_start_run`
/// (même frame) soient flushés, `Query<&ArenaBot>::iter().count() == 0` →
/// break déclenché à tort → wave 1 cleared instantanément (log montrait
/// `Wave 1 spawned` puis `Wave 1 cleared` à 78µs d'écart). Le gate exige
/// d'avoir VU au moins 1 frame avec `alive > 0` avant de pouvoir clear.
#[allow(clippy::too_many_arguments)]
pub fn sys_wave_orchestrator(
    time: Res<Time>,
    mut wave: ResMut<RogueliteWave>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_bots: Query<&ArenaBot>,
    mut open_coffre: MessageWriter<OpenCoffreRequest>,
    // 2026-08-04 — le franchissement de porte, écrit par le bouton du HUD.
    mut enter_room: MessageReader<EnterNextRoomRequest>,
    // 2026-08-04 — en boucle de chapitre, le boss scelle la run ici même.
    mut end_run: MessageWriter<crate::run::EndRunEvent>,
    // Story-571 — gain de Souls méta en fin de wave/boss (persistant).
    mut meta: ResMut<crate::run::MetaSouls>,
    // Story-640 P0-2 — configs live pour le spawn (stats hot-reload + défense).
    // Story-669 — + la composition, en un seul SystemParam (cf `WaveSpawnConfigs`).
    spawn_cfgs: WaveSpawnConfigs,
    // Story-646 R2 — multi-salles : structure de run (genome) + graph (kinds) +
    // transitions RunState (le dispatch d'arène crypts/forge suit tout seul).
    graph_cfg: Res<forgia_stage::graph::RunGraphConfig>,
    graph: Option<Res<forgia_stage::graph::RunGraph>>,
    mut next_run: ResMut<NextState<crate::run::RunState>>,
    // Story-669 — la graine de RUN pilote enfin le placement des ennemis.
    run_seed: Option<Res<crate::run::RunSeed>>,
    // Story-672 — emprises du décor : un ennemi ne doit jamais naître dedans.
    obstacles: Res<crate::decor::DecorObstacles>,
) {
    let alive = q_bots.iter().count() as u32;
    wave.bots_alive = alive;

    if alive > 0 {
        wave.seen_alive = true;
    }

    // Victory déjà émise → no-op (évite spam events).
    if wave.victory_emitted {
        return;
    }

    // Story-677 — en BOUCLE DE ROUNDS, il n'y a pas de parcours : les arènes
    // s'enchaînent, la difficulté monte, et la run s'arrête quand le joueur
    // tombe. `max_rounds = 0` = infini, donc pas de round de boss (le boss
    // deviendra un JALON périodique, il n'est pas encore recâblé — cf story).
    let loop_mode = spawn_cfgs.rounds.enabled;
    // Story-646 — profondeur du boss depuis le graph (fallback config si absent).
    let boss_stage = if loop_mode {
        if spawn_cfgs.rounds.max_rounds == 0 {
            u8::MAX
        } else {
            // 2026-08-04 — `max_rounds` compte des ROUNDS (combats), `boss_stage`
            // une ARÈNE. Les confondre plaçait le boss à l'arène 10, soit le
            // round 31 avec 3 combats par arène.
            //
            // Le boss occupe sa PROPRE arène, juste après les rounds de combat :
            // rounds 1..max_rounds-1 remplissent ⌈(max_rounds-1)/wps⌉ arènes,
            // et le boss est la suivante. Avec 10 rounds et 3 par arène :
            // arènes 0,1,2 portent les rounds 1..9, l'arène 3 porte le boss.
            boss_arena_for(spawn_cfgs.rounds.max_rounds, graph_cfg.waves_per_stage)
        }
    } else {
        graph
            .as_deref()
            .map(|g| g.boss_depth())
            .unwrap_or_else(|| graph_cfg.total_stages.saturating_sub(1))
    };
    let in_boss_stage = wave.stage >= boss_stage;

    // Détection de clear — ARMÉE seulement si : bots vus vivants puis tous morts,
    // pas déjà en break, et PAS en attente de choix de porte (fix boucle infinie
    // 2026-07-02 : le portail laissait seen_alive=true → re-clear → re-break →
    // re-Coffre → +5 Âmes toutes les 15 s, la run ne quittait jamais la salle 0).
    if clear_detection_armed(
        alive,
        wave.seen_alive,
        wave.in_break,
        !wave.portal_choices.is_empty(),
    ) {
        // Vague nettoyée — démarre break ou victory.
        if in_boss_stage {
            // Story-571 — bonus Souls méta pour le boss/finale (persistant).
            let gain = recolte(crate::run::SOULS_PER_BOSS, spawn_cfgs.mods.loot_gain_mul);
            meta.current = meta.current.saturating_add(gain);
            meta.earned_run = meta.earned_run.saturating_add(gain);
            // Story-603 — décision user 2026-06-17 : PLUS d'écran Victoire auto.
            // Tuer le boss ouvre la porte du socle (`loot_room::sys_reconcile_boss_gate`
            // lit `boss_defeated`). `victory_emitted` reste le latch qui stoppe
            // l'orchestrateur (no-op au prochain tick) + gèle `obs_roguelite_player_death`.
            // Boucle : boss → porte → parcours → portail Retour → arène. Condition de
            // fin de run à brancher plus tard.
            wave.victory_emitted = true;
            wave.boss_defeated = true;
            // 2026-08-04 — en BOUCLE DE CHAPITRE, le boss EST la fin.
            //
            // Story-603 avait retiré l'écran de victoire automatique : tuer le
            // boss ouvrait la porte du socle, et la victoire n'était scellée
            // qu'au **retour du parcours** (`loot_room.rs`, portail Retour).
            // Ce parcours n'existe pas dans un chapitre : on tuait le boss et il
            // ne se passait plus rien — `boss_defeated: true` mais
            // `victories_total` figé, observé en jeu.
            //
            // Le chemin story-603 reste intact hors boucle (mode graphe) : c'est
            // `loot_room` qui scelle, comme avant. Ici, la fin de chapitre est le
            // boss, donc c'est ici qu'on la déclare.
            if loop_mode {
                end_run.write(crate::run::EndRunEvent {
                    result: crate::run::RunResult::Victory,
                });
                info!(
                    "[roguelite] Round {} — BOSS VAINCU : CHAPITRE TERMINÉ (+{} Souls méta)",
                    wave.round(graph_cfg.waves_per_stage),
                    crate::run::SOULS_PER_BOSS
                );
                return;
            }
            info!(
                "[roguelite] Salle boss {} nettoyée — BOSS DEFEATED (+{} Souls méta) → porte du socle s'ouvre",
                wave.stage + 1,
                crate::run::SOULS_PER_BOSS
            );
            return;
        }
        // Story-670 — salle SANS COMBAT (Repos) : pas d'Âmes de vague (rien n'a été
        // tué), mais le Coffre est OFFERT. Sans ça la salle ne vaudrait rien : les PV
        // sont déjà restaurés à chaque break, donc « se reposer » ne rendrait aucune
        // ressource. C'est le feu de camp de Slay the Spire.
        let is_rest = !crate::wave_comp::room_spawns_enemies(&spawn_cfgs.comp, wave.room_kind);
        if is_rest {
            wave.in_break = true;
            wave.break_secs_left = BREAK_SECS;
            commands.queue(|world: &mut World| {
                let mut q =
                    world.query_filtered::<&mut forgia_damage::Health, With<forgia_player::Player>>();
                if let Ok(mut hp) = q.single_mut(world) {
                    hp.current = hp.max;
                }
            });
            open_coffre.write(OpenCoffreRequest::rest());
            info!(
                "[roguelite] Salle {} — REPOS : aucun combat, atout OFFERT",
                wave.stage + 1
            );
            return;
        }
        // Story-571 — Souls méta pour une wave régulière nettoyée (persistant).
        let gain = recolte(crate::run::SOULS_PER_WAVE, spawn_cfgs.mods.loot_gain_mul);
        meta.current = meta.current.saturating_add(gain);
        meta.earned_run = meta.earned_run.saturating_add(gain);
        wave.in_break = true;
        wave.break_secs_left = BREAK_SECS;
        // Story-558 AC10 (2026-05-29) — HP restauré à 100% à l'entrée break.
        // Pattern Hadès "Charon's Boon" : sanctuary moment + window prep.
        // Bible cartoon : encourage risk-taking, pas de save-HP-for-next-wave.
        // commands.queue car forgia_damage::Health pas accessible en SystemParam
        // direct (cf miror pattern sys_start_run run.rs:446-451).
        commands.queue(|world: &mut World| {
            let mut q =
                world.query_filtered::<&mut forgia_damage::Health, With<forgia_player::Player>>();
            if let Ok(mut hp) = q.single_mut(world) {
                hp.current = hp.max;
            }
        });
        // Story-558 Phase 3 (2026-05-29) — ouvre le Coffre du Forgeron. UI
        // (forgia-ui-lib::hud::coffre_forgeron) lit CoffreSession populée par
        // sys_handle_open_coffre (forgia-rpg-data::boons).
        open_coffre.write(OpenCoffreRequest::wave_clear());
        info!(
            "[roguelite] Wave {} cleared — break {BREAK_SECS}s before wave {} (HP restored, Coffre opened)",
            wave.current_wave,
            wave.current_wave + 1
        );
    }

    if wave.in_break {
        // 2026-08-04 — DEUX transitions différentes, deux règles.
        //
        // Entre deux combats d'une MÊME arène : le minuteur de 15 s court, c'est
        // la fenêtre de prep (munitions, soin, Coffre du Forgeron).
        //
        // Pour CHANGER d'arène : le minuteur est gelé et on attend que le joueur
        // franchisse la porte. « Tant qu'on ne clique pas, ça n'enchaîne pas » —
        // une pièce se quitte parce qu'on l'a décidé, pas parce qu'un compte à
        // rebours s'est vidé pendant qu'on lisait ses atouts.
        let arena_change_is_next = wave.current_wave >= graph_cfg.waves_per_stage;
        if loop_mode && arena_change_is_next {
            wave.awaiting_room_entry = true;
            if enter_room.read().next().is_none() {
                return; // toujours devant la porte
            }
            // Porte franchie : le clic VAUT la fin du break, le reste du chemin
            // (avance d'arène + spawn) est inchangé.
            wave.awaiting_room_entry = false;
            wave.break_secs_left = 0.0;
        } else {
            wave.break_secs_left -= time.delta_secs();
        }
        if wave.break_secs_left <= 0.0 {
            wave.in_break = false;
            wave.break_secs_left = 0.0;
            if wave.current_wave < graph_cfg.waves_per_stage {
                // Vague suivante dans la MÊME salle.
                wave.current_wave += 1;
            } else {
                // Story-646 R2 — salle nettoyée → SALLE SUIVANTE.
                // `wave.stage` est un u8 : en boucle infinie, le round 255 est
                // un plafond DUR de la représentation, pas un choix de design.
                // `saturating_add` évite l'overflow ; le round 255 déclenche le
                // boss (boss_stage = u8::MAX) et scelle la run.
                let next = wave.stage.saturating_add(1);
                let is_boss = next >= boss_stage;
                // Inc.2 — portes candidates : kinds des variants du graph au depth
                // suivant (cap `branching`). Boss = chemin unique, jamais de choix.
                let choices: Vec<forgia_stage::graph::StageKind> = if is_boss || loop_mode {
                    // En boucle de rounds : AUCUN choix de porte. C'est ce que
                    // « on ne branche pas le parcours » veut dire concrètement —
                    // le graphe n'est pas consulté, les arènes s'enchaînent.
                    Vec::new()
                } else {
                    graph
                        .as_deref()
                        .and_then(|g| g.stages.get(next as usize))
                        .map(|variants| {
                            variants
                                .iter()
                                .map(|n| n.kind)
                                .take(graph_cfg.branching.max(1) as usize)
                                .collect()
                        })
                        .unwrap_or_default()
                };
                if choices.len() >= 2 {
                    // Inc.2 — CHOIX DE PORTE : on gèle ici. L'overlay
                    // (`hud::draw_portal_overlay`) affiche les portes ; le pick
                    // (portal_pick) est consommé plus bas au tick suivant.
                    // seen_alive=false : ceinture+bretelles avec le guard
                    // `clear_detection_armed` (anti re-clear pendant l'attente).
                    info!(
                        "[roguelite] Salle {} nettoyée — CHOISIS TA PORTE : {:?}",
                        wave.stage + 1,
                        choices,
                    );
                    wave.portal_choices = choices;
                    wave.seen_alive = false;
                    return;
                }
                // Pas de choix possible (boss / graph absent / 1 seul variant) → auto.
                // Story-677 — en boucle, le TYPE de salle vient du rythme déclaré
                // (respiration tous les N rounds), pas d'un nœud de graphe.
                let (kind, budget) = if loop_mode {
                    let k = if spawn_cfgs.rounds.is_respite_round(u32::from(next)) {
                        Some(forgia_stage::graph::StageKind::Rest)
                    } else {
                        Some(forgia_stage::graph::StageKind::Combat)
                    };
                    (k, graph_cfg.director_budget_for_depth(next))
                } else {
                    (
                        choices.first().copied(),
                        node_budget(graph.as_deref(), &graph_cfg, next, 0),
                    )
                };
                advance_to_room(&mut wave, next, is_boss, kind, budget);
                if is_boss {
                    next_run.set(crate::run::RunState::Boss { stage: wave.stage });
                } else {
                    next_run.set(crate::run::RunState::InRun { stage: wave.stage });
                }
                info!(
                    "[roguelite] → SALLE {}/{} ({}{:?})",
                    wave.stage + 1,
                    boss_stage + 1,
                    if is_boss { "BOSS " } else { "" },
                    wave.room_kind,
                );
            }
            let density = crate::wave_comp::density_from_budget(
                wave.room_budget,
                graph_cfg.director_budget_for_depth(0),
            );
            spawn_wave_enemies(
                &mut commands,
                &asset_server,
                &spawn_cfgs.ctx(
                    wave.current_wave,
                    wave.stage,
                    wave.room_kind,
                    density,
                    run_seed.as_ref().map(|s| s.seed).unwrap_or(0),
                    &obstacles,
                ),
            );
            // Reset gate : la nouvelle wave doit prouver alive>0 avant pouvoir clear.
            wave.seen_alive = false;
            // ...SAUF si la salle ne spawne rien. Ce reset s'exécutait APRÈS
            // `arm_non_combat_room` et ANNULAIT son armement : une salle de
            // repos se retrouvait sans ennemi ET sans `seen_alive`, donc la
            // détection de clear ne s'armait jamais — la run FIGEAIT dedans.
            // Le défaut ne se voyait pas tant que le Repos était rare ; la
            // boucle de rounds en pose un tous les N rounds, et la soak l'a
            // attrapé. L'armement doit donc venir APRÈS le reset, pas avant.
            arm_non_combat_room(&mut wave, &spawn_cfgs.comp, graph_cfg.waves_per_stage);
        }
        return;
    }

    // Story-646 Inc.2 — le joueur a choisi sa porte (overlay hud) : transition + spawn.
    if let Some(pick) = wave.portal_pick.take() {
        if wave.portal_choices.is_empty() {
            return; // pick orphelin (reset de run pendant le choix) — no-op.
        }
        let kind = wave
            .portal_choices
            .get(pick as usize)
            .or_else(|| wave.portal_choices.first())
            .copied();
        wave.portal_choices.clear();
        let next = wave.stage + 1;
        // Story-669 — le budget du nœud RÉELLEMENT choisi pilote la densité.
        let budget = node_budget(graph.as_deref(), &graph_cfg, next, pick as usize);
        advance_to_room(&mut wave, next, false, kind, budget);
        arm_non_combat_room(&mut wave, &spawn_cfgs.comp, graph_cfg.waves_per_stage);
        next_run.set(crate::run::RunState::InRun { stage: wave.stage });
        let density =
            crate::wave_comp::density_from_budget(budget, graph_cfg.director_budget_for_depth(0));
        info!(
            "[roguelite] → SALLE {}/{} — porte choisie : {:?} (densité ×{:.2})",
            wave.stage + 1,
            boss_stage + 1,
            kind,
            spawn_cfgs.comp.density_factor(density),
        );
        spawn_wave_enemies(
            &mut commands,
            &asset_server,
            &spawn_cfgs.ctx(
                wave.current_wave,
                wave.stage,
                wave.room_kind,
                density,
                run_seed.as_ref().map(|s| s.seed).unwrap_or(0),
                &obstacles,
            ),
        );
    }
}

/// Story-670 — prépare une salle SANS COMBAT dès qu'on y entre.
///
/// Deux choses, et les deux sont indispensables :
/// 1. `seen_alive = true` — la détection de clôture exige d'avoir VU des ennemis
///    vivants. Aucun ne viendra, donc sans ça `clear_detection_armed` ne s'arme
///    jamais et **la run se fige**. C'était le vrai obstacle derrière le plancher
///    d'1 ennemi que story-669 avait posé faute de mieux.
/// 2. `current_wave` sur la DERNIÈRE vague de la salle — sinon la fin du break
///    enchaînerait sur la vague 2 de la même salle de Repos, qui offrirait un
///    second atout gratuit, puis un troisième…
fn arm_non_combat_room(wave: &mut RogueliteWave, comp: &WaveCompConfig, waves_per_stage: u8) {
    if crate::wave_comp::room_spawns_enemies(comp, wave.room_kind) {
        return;
    }
    wave.seen_alive = true;
    wave.current_wave = waves_per_stage.max(1);
}

/// Budget de difficulté du nœud de graph `(depth, variant)`, avec repli sur la
/// formule du director si le graph est absent. C'est le premier consommateur de
/// `StageNode.difficulty_budget`, calculé à chaque run depuis story-470 et jeté.
fn node_budget(
    graph: Option<&forgia_stage::graph::RunGraph>,
    cfg: &forgia_stage::graph::RunGraphConfig,
    depth: u8,
    variant: usize,
) -> u32 {
    graph
        .and_then(|g| g.stages.get(depth as usize))
        .and_then(|v| v.get(variant).or_else(|| v.first()))
        .map(|n| n.difficulty_budget_centi)
        .unwrap_or_else(|| cfg.director_budget_for_depth(depth))
}

/// PUR (testable) — la détection « vague nettoyée » n'est armée que si : tous les
/// bots (vus vivants) sont morts, PAS en break, PAS en attente de porte. Le 4e
/// terme est le fix de la boucle infinie du 2026-07-02 (re-clear pendant le
/// choix de porte → re-break/re-Coffre/farm d'Âmes, salle jamais quittée).
pub fn clear_detection_armed(
    alive: u32,
    seen_alive: bool,
    in_break: bool,
    awaiting_portal: bool,
) -> bool {
    alive == 0 && seen_alive && !in_break && !awaiting_portal
}

/// Story-646 — avance `RogueliteWave` vers la salle `next` (compteurs + kind).
/// La transition `RunState` + le spawn restent au caller (params Bevy).
fn advance_to_room(
    wave: &mut RogueliteWave,
    next: u8,
    is_boss: bool,
    kind: Option<forgia_stage::graph::StageKind>,
    budget: u32,
) {
    wave.stage = next;
    wave.seen_alive = false;
    wave.is_boss_arena = is_boss;
    wave.room_kind = kind;
    // Story-669 — le budget du nœud de graph choisi pilote la densité de la salle.
    wave.room_budget = budget;
    wave.current_wave = if is_boss { BOSS_WAVE_COMPOSITION } else { 1 };
}

/// M3 step 1 — marker enrage phase 2. Inséré quand HP boss < 50%.
#[derive(Component, Default)]
pub struct BossEnraged;

/// Story-558 P3 (2026-05-29) — Message fired par sys_boss_enrage au moment
/// du trigger (transition Without<BossEnraged> → With). Consommé par UI
/// banner + camera shake punch.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossEnrageTriggeredEvent;

/// Détecte boss à ≤50% HP → insert BossEnraged + boost stats AI runtime +
/// fire `BossEnrageTriggeredEvent` (P3 telegraph visuel).
/// Idempotent : `Without<BossEnraged>` filtre évite re-trigger.
pub fn sys_boss_enrage(
    mut commands: Commands,
    mut q_boss: Query<(Entity, &Health, &EnemyArchetype, &mut ArenaBot), Without<BossEnraged>>,
    mut enrage_w: MessageWriter<BossEnrageTriggeredEvent>,
) {
    for (entity, health, archetype, mut bot) in &mut q_boss {
        if *archetype != EnemyArchetype::Boss {
            continue;
        }
        // Story-490 — forgia_combat::Health n'a pas .fraction() ; inline calcul
        // (équivalent à forgia_damage::Health::fraction).
        let fraction = if health.max > 0.0 {
            (health.current / health.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if fraction <= 0.5 {
            let stats = enemies::stats_for(EnemyArchetype::Boss);
            bot.speed = stats.speed * 1.8;
            bot.attack_cooldown = stats.attack_cooldown * 0.55;
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.insert(BossEnraged);
            }
            // P3 telegraph — fire event consommé par UI banner + camera shake.
            enrage_w.write(BossEnrageTriggeredEvent);
            info!(
                "[roguelite] BOSS ENRAGED — phase 2 (HP {:.0}%, speed {:.1}, cooldown {:.2}s)",
                fraction * 100.0,
                bot.speed,
                bot.attack_cooldown
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    fn spawn_first_wave_for_qa(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        cfgs: WaveSpawnConfigs,
        obstacles: Res<crate::decor::DecorObstacles>,
        mut spawned: Local<bool>,
    ) {
        if *spawned {
            return;
        }
        // Salle 0, type Combat, densité de référence, graine fixe : la QA doit
        // rester déterministe.
        spawn_wave_enemies(
            &mut commands,
            &asset_server,
            &cfgs.ctx(1, 0, Some(forgia_stage::graph::StageKind::Combat), 1.0, 0, &obstacles),
        );
        *spawned = true;
    }

    /// Story-669 — la composition est passée en couche definition ; ces deux tests
    /// vérifient désormais que l'ÉQUILIBRE DE RÉFÉRENCE n'a pas bougé au passage
    /// (salle 0, type Combat, densité 1.0 → exactement l'ancienne table Rust).
    #[test]
    fn wave_composition_grows() {
        use forgia_stage::graph::StageKind;
        let cfg = WaveCompConfig::default();
        let sum = |w: u8| -> u32 {
            crate::wave_comp::compose(&cfg, w, Some(StageKind::Combat), 1.0)
                .iter()
                .map(|(_, c, _)| *c)
                .sum()
        };
        let (w1, w2, w3) = (sum(1), sum(2), sum(3));
        assert!(w1 < w2, "wave 2 doit avoir plus d'ennemis que wave 1");
        assert_eq!(w1, 8);
        assert_eq!(w2, 12);
        assert_eq!(w3, 5, "wave 3 = 1 boss + 4 support");
    }

    #[test]
    fn wave_3_contains_boss() {
        let w3 = crate::wave_comp::compose(&WaveCompConfig::default(), 3, None, 1.0);
        assert!(
            w3.iter().any(|(a, _, _)| *a == EnemyArchetype::Boss),
            "wave 3 doit contenir un Boss"
        );
    }

    /// Story-670 — les deux pièges d'une salle sans combat, verrouillés.
    #[test]
    fn a_non_combat_room_is_armed_on_entry() {
        let comp = WaveCompConfig::default();
        let mut w = RogueliteWave {
            room_kind: Some(forgia_stage::graph::StageKind::Rest),
            current_wave: 1,
            seen_alive: false,
            ..Default::default()
        };
        arm_non_combat_room(&mut w, &comp, 2);
        assert!(
            w.seen_alive,
            "sans ça, aucun ennemi n'arrive, la clôture ne s'arme jamais → RUN FIGÉE"
        );
        assert_eq!(
            w.current_wave, 2,
            "la salle doit être sur sa DERNIÈRE vague, sinon le Repos se rejoue \
             et offre un atout gratuit par vague"
        );
    }

    #[test]
    fn a_combat_room_is_left_untouched_by_the_arming() {
        let comp = WaveCompConfig::default();
        let mut w = RogueliteWave {
            room_kind: Some(forgia_stage::graph::StageKind::Elite),
            current_wave: 1,
            seen_alive: false,
            ..Default::default()
        };
        arm_non_combat_room(&mut w, &comp, 2);
        assert!(!w.seen_alive, "une salle de combat garde son gate anti-race");
        assert_eq!(w.current_wave, 1);
    }

    /// La structure décidée le 2026-08-04, round par round. Si cette table change,
    /// c'est le chapitre entier qui change de forme — elle doit se lire d'un coup.
    #[test]
    fn a_chapter_maps_rounds_onto_arenas() {
        // (arène, vague) → round attendu, avec 3 combats par arène.
        let cases = [
            ((0u8, 1u8), 1u32),
            ((0, 2), 2),
            ((0, 3), 3), // fin de l'arène 1 → palier + changement de décor
            ((1, 1), 4),
            ((1, 3), 6),
            ((2, 1), 7),
            ((2, 3), 9),
            ((3, 1), 10), // l'arène du BOSS
        ];
        for ((stage, current_wave), expected) in cases {
            let w = RogueliteWave {
                stage,
                current_wave,
                ..Default::default()
            };
            assert_eq!(w.round(3), expected, "arène {stage}, vague {current_wave}");
        }
    }

    /// Le round est un COMBAT, l'arène en contient plusieurs. Les confondre
    /// faisait afficher « ROUND 1 / 10 » trois fois de suite et montrer la menace
    /// par bonds de trois.
    #[test]
    fn the_round_is_not_the_arena() {
        let w = RogueliteWave {
            stage: 2,
            current_wave: 2,
            ..Default::default()
        };
        assert_eq!(w.round(3), 8);
        assert_ne!(w.round(3), u32::from(w.stage), "round ≠ index d'arène");
        // Un seul combat par arène : les deux coïncident, et c'est le seul cas.
        assert_eq!(
            RogueliteWave {
                stage: 5,
                current_wave: 1,
                ..Default::default()
            }
            .round(1),
            6
        );
    }

    /// `waves_per_stage = 0` serait une division par zéro dans la dérivation.
    #[test]
    fn a_zero_wave_arena_is_treated_as_one() {
        let w = RogueliteWave {
            stage: 3,
            current_wave: 1,
            ..Default::default()
        };
        assert_eq!(w.round(0), w.round(1));
    }

    /// Le boss occupe sa propre arène, juste après les rounds de combat.
    #[test]
    fn the_boss_gets_its_own_arena_after_the_combat_rounds() {
        // La configuration livrée : 10 rounds, 3 par arène.
        assert_eq!(boss_arena_for(10, 3), 3);
        // …et le round de cette arène EST le dernier du chapitre.
        let boss = RogueliteWave {
            stage: boss_arena_for(10, 3),
            current_wave: 1,
            ..Default::default()
        };
        assert_eq!(boss.round(3), 10);
    }

    /// La dérivation ne doit pas se casser sur les configurations voisines : c'est
    /// un gène, quelqu'un le changera.
    #[test]
    fn the_boss_arena_holds_for_other_chapter_shapes() {
        assert_eq!(boss_arena_for(10, 1), 9, "1 combat par arène → 9 arènes puis le boss");
        assert_eq!(boss_arena_for(4, 3), 1, "3 combats puis le boss");
        assert_eq!(boss_arena_for(1, 3), 0, "chapitre réduit au boss");
        assert_eq!(boss_arena_for(0, 3), 0, "pas de round de combat");
        assert_eq!(boss_arena_for(11, 3), 4, "10 rounds de combat → 4 arènes");
    }

    /// **LE test qui interdit un sixième dédoublement du concept « round ».**
    ///
    /// Cinq consommateurs ont lu `wave.stage` en croyant lire un round, et chacun
    /// a été trouvé séparément, en jeu, après coup :
    ///
    /// | # | Consommateur | Ce que le défaut produisait |
    /// |---|---|---|
    /// | 1 | compteur du HUD | « ROUND 1 / 10 » pendant trois combats |
    /// | 2 | paliers / répit | la marche ne tombait pas où l'écran l'annonçait |
    /// | 3 | montée de menace | la difficulté montait par bonds de trois |
    /// | 4 | menace AFFICHÉE | l'écran annonçait ×1,0 pendant qu'on encaissait ×1,6 |
    /// | 5 | chrono de rythme | 114 s d'arène comparées à 90 s de budget de COMBAT |
    ///
    /// Le cinquième a produit un « TU DÉCROCHES » à un joueur qui était à 40 % du
    /// budget. C'est la classe entière qui est en cause, pas les occurrences :
    /// tant qu'une grandeur a deux définitions, on en trouvera une sixième.
    ///
    /// Ce test fixe la seule définition. Toute nouvelle lecture doit lui répondre.
    #[test]
    fn the_round_has_exactly_one_definition() {
        let wps = 3u8;
        // La table de vérité, arène par arène. Aucune autre formule n'a le droit
        // de produire ces nombres.
        for stage in 0u8..4 {
            for wave_idx in 1u8..=wps {
                let w = RogueliteWave {
                    stage,
                    current_wave: wave_idx,
                    ..Default::default()
                };
                let attendu = u32::from(stage) * u32::from(wps) + u32::from(wave_idx);
                assert_eq!(w.round(wps), attendu, "arène {stage}, combat {wave_idx}");
                // L'erreur commise cinq fois : prendre l'arène pour le round.
                if stage > 0 || wave_idx > 1 {
                    assert_ne!(
                        w.round(wps),
                        u32::from(stage),
                        "arène {stage} confondue avec un round — c'est le défaut de 2026-08-04"
                    );
                }
            }
        }
        // Un round est 1-basé : personne n'a jamais joué un « round 0 ».
        assert_eq!(RogueliteWave::default().round(wps), 1);
    }

    /// Le boss se bat au round 10, pas au round 12 (2026-08-04, observé en jeu).
    ///
    /// `current_wave` est surchargé dans l'arène du boss : il y porte
    /// `BOSS_WAVE_COMPOSITION` (= 3), une COMPOSITION, pas un indice de combat.
    /// Lu comme un indice, il donnait `3×3 + 3 = 12` — et la menace appliquée
    /// aux ennemis passait de ×6,91 à ×9,29. Le combat de boss tournait **35 %
    /// trop dur**, sans que le HUD le montre (son compteur est clampé au total).
    #[test]
    fn the_boss_fights_on_the_last_round_not_past_it() {
        let boss = RogueliteWave {
            stage: boss_arena_for(10, 3),
            current_wave: BOSS_WAVE_COMPOSITION,
            is_boss_arena: true,
            ..Default::default()
        };
        assert_eq!(boss.round(3), 10, "le boss ferme le chapitre, il ne le dépasse pas");

        // Sans le drapeau, on retombe sur le défaut : la preuve que c'est bien
        // lui qui sépare « composition » de « indice de combat ».
        let sans_drapeau = RogueliteWave {
            is_boss_arena: false,
            ..boss
        };
        assert_eq!(sans_drapeau.round(3), 12, "c'est exactement le défaut observé");
    }

    /// Et l'arène du boss se marque toute seule en y entrant — personne n'a à
    /// penser à poser le drapeau.
    #[test]
    fn entering_the_boss_arena_marks_it() {
        let mut w = RogueliteWave::default();
        advance_to_room(&mut w, 3, true, None, 0);
        assert!(w.is_boss_arena);
        assert_eq!(w.round(3), 10);
        // Et une arène ordinaire ne se marque pas.
        advance_to_room(&mut w, 1, false, None, 0);
        assert!(!w.is_boss_arena);
        assert_eq!(w.round(3), 4);
    }

    /// Une arène ne se quitte pas toute seule (2026-08-04).
    ///
    /// Le minuteur reste la fenêtre de prep ENTRE deux combats d'une même arène ;
    /// changer de pièce attend le joueur. Ce test fixe la frontière entre les deux
    /// — c'est elle qui décide si la run avance sur un compte à rebours ou sur une
    /// décision.
    #[test]
    fn the_last_fight_of_an_arena_waits_for_the_player() {
        let wps = 3u8;
        // Combats 1 et 2 : ce n'est pas encore une sortie d'arène.
        for wave in 1u8..wps {
            let w = RogueliteWave {
                current_wave: wave,
                ..Default::default()
            };
            assert!(
                w.current_wave < wps,
                "combat {wave} : le minuteur enchaîne, on reste dans la pièce"
            );
        }
        // Dernier combat de l'arène : c'est une PORTE.
        let w = RogueliteWave {
            current_wave: wps,
            ..Default::default()
        };
        assert!(
            w.current_wave >= wps,
            "dernier combat : la suite est un changement d'arène, donc un clic"
        );
    }

    /// L'attente est un état LU par le HUD : sans ça, le joueur reste devant une
    /// porte invisible et croit que la run a planté.
    #[test]
    fn waiting_at_the_door_is_observable() {
        let w = RogueliteWave::default();
        assert!(!w.awaiting_room_entry, "on ne naît pas devant une porte");
        let waiting = RogueliteWave {
            awaiting_room_entry: true,
            ..Default::default()
        };
        assert!(waiting.awaiting_room_entry);
    }

    #[test]
    fn wave_default_state() {
        let w = RogueliteWave::default();
        assert_eq!(w.current_wave, 1);
        assert_eq!(w.bots_alive, 0);
        assert!(!w.in_break);
        assert!(!w.victory_emitted);
        assert!(!w.boss_defeated);
    }

    #[test]
    fn clear_detection_blocked_while_awaiting_portal() {
        // Régression 2026-07-02 : pendant l'attente de porte (bots morts, pas de
        // break), la détection de clear ne doit PAS re-fire (boucle infinie
        // break→Coffre→Âmes). Le 4e terme la désarme.
        assert!(clear_detection_armed(0, true, false, false), "clear normal");
        assert!(
            !clear_detection_armed(0, true, false, true),
            "en attente de porte → désarmée (LE bug)"
        );
        assert!(
            !clear_detection_armed(0, true, true, false),
            "en break → désarmée"
        );
        assert!(
            !clear_detection_armed(0, false, false, false),
            "gate anti-race"
        );
        assert!(
            !clear_detection_armed(3, true, false, false),
            "bots vivants"
        );
    }

    #[test]
    fn wave_default_starts_room_zero_gated() {
        // Story-646 R2 — départ salle 0, gate anti-race baissé (aucun bot vu).
        let w = RogueliteWave::default();
        assert_eq!(w.stage, 0);
        assert!(!w.seen_alive);
        assert_eq!(
            BOSS_WAVE_COMPOSITION, 3,
            "compos boss = branche _ de wave_composition"
        );
    }

    #[test]
    fn break_secs_positive() {
        assert!(BREAK_SECS > 0.0);
    }

    /// Régression visuelle : une vague jouable doit conserver un rig KayKit par
    /// ennemi. Ce test est volontairement headless : il contrôle le contrat ECS
    /// de spawn sans imposer une progression manuelle jusqu'à l'arène.
    #[test]
    fn qa_wave_one_spawns_an_animated_scene_for_every_enemy() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Scene>();
        app.insert_resource(EnemyStatsConfig::default());
        app.insert_resource(DefenseConfig::default());
        // Story-669 — la composition vient d'une Resource ; le harness QA ne monte
        // pas le plugin (donc pas `sys_init_wave_comp_genome`) : on pose le miroir.
        app.insert_resource(WaveCompConfig::default());
        // Story-677 — `WaveSpawnConfigs` porte désormais la config de boucle.
        app.insert_resource(crate::rounds::RoundsConfig::default());
        app.insert_resource(crate::decor::DecorObstacles::default());
        // 2026-08-04 — les atouts « récolte » vivent dans le bundle de spawn.
        app.insert_resource(forgia_combat::combat_mods::PlayerCombatMods::default());
        app.add_systems(Update, spawn_first_wave_for_qa);

        app.update();

        let enemy_count = app
            .world_mut()
            .query_filtered::<Entity, With<ArenaBot>>()
            .iter(app.world())
            .count();
        let visuals: Vec<String> = app
            .world_mut()
            .query_filtered::<&Name, With<SceneRoot>>()
            .iter(app.world())
            .map(ToString::to_string)
            .collect();

        assert_eq!(enemy_count, 8, "la vague 1 doit créer ses 8 bots");
        assert_eq!(
            visuals.len(),
            enemy_count,
            "chaque bot doit avoir un SceneRoot animé"
        );
        assert!(
            visuals.iter().all(|name| name.contains("_visual")),
            "aucun visuel de vague ne doit être un proxy statique: {visuals:?}"
        );
    }

    /// QA headless du flux de combat : une vague réellement vue vivante, puis
    /// vidée, doit ouvrir le break et créer la vague suivante. Cela remplace le
    /// trajet manuel jusqu'à l'arène pour cette régression de progression.
    #[test]
    fn qa_wave_clear_transitions_to_the_next_wave() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Scene>();
        app.insert_resource(EnemyStatsConfig::default());
        app.insert_resource(DefenseConfig::default());
        // Story-669 — la composition vient d'une Resource ; le harness QA ne monte
        // pas le plugin (donc pas `sys_init_wave_comp_genome`) : on pose le miroir.
        app.insert_resource(WaveCompConfig::default());
        // Story-677 — `WaveSpawnConfigs` porte désormais la config de boucle.
        app.insert_resource(crate::rounds::RoundsConfig::default());
        app.insert_resource(crate::decor::DecorObstacles::default());
        // 2026-08-04 — les atouts « récolte » vivent dans le bundle de spawn.
        app.insert_resource(forgia_combat::combat_mods::PlayerCombatMods::default());
        app.insert_resource(RogueliteWave::default());
        app.insert_resource(crate::run::MetaSouls::default());
        app.insert_resource(forgia_stage::graph::RunGraphConfig::default());
        app.insert_resource(NextState::<crate::run::RunState>::default());
        app.add_message::<OpenCoffreRequest>();
        app.add_message::<EnterNextRoomRequest>();
        app.add_message::<crate::run::EndRunEvent>();
        app.add_systems(Update, sys_wave_orchestrator);

        let bot = app.world_mut().spawn(ArenaBot::default()).id();
        app.update();
        assert!(app.world().resource::<RogueliteWave>().seen_alive);

        app.world_mut().entity_mut(bot).despawn();
        app.update();
        {
            let wave = app.world().resource::<RogueliteWave>();
            assert!(wave.in_break, "une vague vidée doit démarrer son break");
            assert_eq!(wave.current_wave, 1);
        }

        app.world_mut()
            .resource_mut::<RogueliteWave>()
            .break_secs_left = 0.0;
        app.update();

        let enemy_count = app
            .world_mut()
            .query_filtered::<Entity, With<ArenaBot>>()
            .iter(app.world())
            .count();
        let wave = app.world().resource::<RogueliteWave>();
        assert!(!wave.in_break);
        assert_eq!(wave.current_wave, 2);
        assert_eq!(enemy_count, 12, "la vague 2 doit être réellement créée");
    }

    /// Soak headless : simule 24 salles mono-vague. Le nombre de bots et de
    /// SceneRoots doit revenir à sa valeur nominale après chaque clear, ce qui
    /// détecte une accumulation d'entités de combat sans demander de parcourir
    /// manuellement les niveaux.
    #[test]
    fn qa_wave_soak_keeps_combat_entities_bounded_across_rooms() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Scene>();
        app.insert_resource(EnemyStatsConfig::default());
        app.insert_resource(DefenseConfig::default());
        // Story-669 — la composition vient d'une Resource ; le harness QA ne monte
        // pas le plugin (donc pas `sys_init_wave_comp_genome`) : on pose le miroir.
        app.insert_resource(WaveCompConfig::default());
        // Story-677 — `WaveSpawnConfigs` porte désormais la config de boucle.
        //
        // Cette soak traverse 24 salles pour prouver l'ABSENCE DE FUITE sur une
        // longue run : elle a besoin d'une boucle SANS fin. Depuis le 2026-08-04
        // le défaut livré est un chapitre borné (`max_rounds = 10`) — la run
        // s'arrêterait en Victoire au round 10 et le test accuserait la run
        // d'être figée alors qu'elle est gagnée. On déclare le mode qu'on teste.
        app.insert_resource(crate::rounds::RoundsConfig {
            max_rounds: 0,
            ..crate::rounds::RoundsConfig::default()
        });
        app.insert_resource(crate::decor::DecorObstacles::default());
        // 2026-08-04 — les atouts « récolte » vivent dans le bundle de spawn.
        app.insert_resource(forgia_combat::combat_mods::PlayerCombatMods::default());
        app.insert_resource(RogueliteWave::default());
        app.insert_resource(crate::run::MetaSouls::default());
        app.insert_resource(forgia_stage::graph::RunGraphConfig {
            total_stages: 26,
            boss_stage_index: 25,
            branching: 1,
            director_credits_base: 2.0,
            director_credits_stage_mult: 1.25,
            waves_per_stage: 1,
        });
        app.insert_resource(NextState::<crate::run::RunState>::default());
        app.add_message::<OpenCoffreRequest>();
        app.add_message::<EnterNextRoomRequest>();
        app.add_message::<crate::run::EndRunEvent>();
        app.add_systems(Update, sys_wave_orchestrator);
        app.add_systems(Update, spawn_first_wave_for_qa);

        app.update();
        // Les Commands du spawn sont appliquées après l'orchestrateur : une
        // frame suivante est nécessaire pour armer `seen_alive`, comme en jeu.
        app.update();
        // Story-669 — l'effectif n'est plus constant (la densité monte avec la
        // profondeur), donc on ne l'assertionne plus à 8. Ce que ce test garde,
        // et qui est son vrai objet, c'est l'ABSENCE DE FUITE : un visuel par bot,
        // les précédents despawnés, et un effectif qui reste borné.
        //
        // Plafond : `density.max_factor` (2.5) appliqué à la vague 1 (3T/3R/2S)
        // donne au pire 8+8+5 = 21 ennemis. 32 laisse de la marge sans rien cacher.
        const MAX_BOTS_PER_WAVE: usize = 32;
        // L'invariant ANTI-FIGE de cette soak : la run doit avancer. Un blocage
        // se lit ici, quel que soit le type de salle.
        let mut stage_before = app.world().resource::<RogueliteWave>().stage;
        let mut advanced = 0u32;
        for room in 0..24 {
            let bots: Vec<Entity> = app
                .world_mut()
                .query_filtered::<Entity, With<ArenaBot>>()
                .iter(app.world())
                .collect();
            // Story-677 — une salle de RESPIRATION ne spawne rien, par
            // construction (relâche tous les N rounds). La cadence de frames de
            // ce test suppose une salle de COMBAT (clear → break → spawn) : sur
            // une respiration elle ne s'applique pas.
            //
            // Ce que le test protège n'est pas affaibli pour autant — il est
            // RENFORCÉ : au lieu de « il y a des bots », on vérifie plus bas que
            // la run AVANCE VRAIMENT (`stage` strictement croissant sur toute la
            // soak). Une salle qui figerait la run bloquerait ce compteur, avec
            // ou sans bots.
            let kind = app.world().resource::<RogueliteWave>().room_kind;
            if bots.is_empty() {
                assert_eq!(
                    kind,
                    Some(forgia_stage::graph::StageKind::Rest),
                    "salle {room}: vague VIDE dans une salle de COMBAT — la run figerait"
                );
                // Le temps n'avance pas tout seul dans une App de test : le
                // break doit être drainé à la main, exactement comme le fait le
                // chemin combat plus bas. Sans ça le test accuse la respiration
                // de figer la run alors que c'est l'horloge qui ne tourne pas.
                for _ in 0..3 {
                    app.update();
                    app.world_mut()
                        .resource_mut::<RogueliteWave>()
                        .break_secs_left = 0.0;
                    app.update();
                    // Une salle de repos est la dernière d'une arène comme une
                    // autre : elle débouche sur une PORTE, pas sur un minuteur.
                    if app.world().resource::<RogueliteWave>().awaiting_room_entry {
                        app.world_mut().write_message(EnterNextRoomRequest);
                        app.update();
                    }
                }
                let after = app.world().resource::<RogueliteWave>().stage;
                assert!(
                    after > stage_before,
                    "salle {room}: la respiration n'a pas fait avancer la run (stage figé à {after})"
                );
                stage_before = after;
                continue;
            }
            assert!(
                bots.len() <= MAX_BOTS_PER_WAVE,
                "salle {room}: effectif non borné ({}) — budget de frame en danger",
                bots.len()
            );
            for bot in bots {
                app.world_mut().entity_mut(bot).despawn();
            }

            app.update();
            assert!(
                app.world().resource::<RogueliteWave>().in_break,
                "salle {room}: clear doit ouvrir un break"
            );
            app.world_mut()
                .resource_mut::<RogueliteWave>()
                .break_secs_left = 0.0;
            app.update();
            // 2026-08-04 — le dernier combat d'une arène ne s'enchaîne PLUS tout
            // seul : il attend que le joueur franchisse la porte. Cette soak joue
            // donc le joueur, sinon elle reste plantée devant et accuse la run
            // d'être figée alors qu'elle attend, correctement, une décision.
            if app.world().resource::<RogueliteWave>().awaiting_room_entry {
                app.world_mut().write_message(EnterNextRoomRequest);
                app.update();
                app.world_mut()
                    .resource_mut::<RogueliteWave>()
                    .break_secs_left = 0.0;
                app.update();
            }

            let scene_roots = app
                .world_mut()
                .query_filtered::<Entity, With<SceneRoot>>()
                .iter(app.world())
                .count();
            let live_bots = app
                .world_mut()
                .query_filtered::<Entity, With<ArenaBot>>()
                .iter(app.world())
                .count();
            assert_eq!(
                scene_roots, live_bots,
                "salle {room}: 1 visuel par bot — les précédents doivent être despawnés"
            );
            // Arme la vague qui vient d'être créée avant le clear suivant.
            app.update();
            let after = app.world().resource::<RogueliteWave>().stage;
            if after > stage_before {
                advanced += 1;
                stage_before = after;
            }
        }
        assert!(
            advanced >= 4,
            "la run n'a avancé que de {advanced} salles en 24 itérations — quelque chose FIGE"
        );
    }
}

// ─── Récolte : le gain d'Âmes des atouts (2026-08-04) ────────────────────────

#[cfg(test)]
mod recolte_tests {
    use super::*;

    /// Sans atout, le gain est exactement celui d'avant — la famille récolte ne
    /// doit rien changer par sa seule existence.
    #[test]
    fn no_boon_means_the_bare_reward() {
        assert_eq!(recolte(crate::run::SOULS_PER_WAVE, 1.0), crate::run::SOULS_PER_WAVE);
        assert_eq!(recolte(crate::run::SOULS_PER_BOSS, 1.0), crate::run::SOULS_PER_BOSS);
    }

    /// Un atout de récolte rapporte VRAIMENT plus.
    #[test]
    fn a_harvest_boon_actually_pays_more() {
        assert_eq!(recolte(100, 1.20), 120);
        assert_eq!(recolte(100, 1.55), 155);
        // Le boss, qui vaut 25, passe à 30 avec +20 %.
        assert_eq!(recolte(crate::run::SOULS_PER_BOSS, 1.20), 30);
    }

    /// **Un bonus ne peut être qu'un bonus.** Un multiplicateur dégénéré ne doit
    /// jamais RETIRER au joueur ce qu'il aurait touché sans atout — c'est le
    /// genre de régression qu'on ne voit qu'après des heures de jeu.
    #[test]
    fn a_degenerate_multiplier_never_takes_anything_away() {
        for mul in [0.0, -5.0, 0.5, f32::NAN, f32::NEG_INFINITY] {
            assert_eq!(recolte(25, mul), 25, "mul {mul}");
        }
    }

    /// Petites récompenses : +20 % sur 5 Âmes doit arrondir à 6, pas retomber à 5
    /// par troncature — sinon l'atout serait invisible sur les gains de vague.
    #[test]
    fn small_rewards_still_feel_the_boon() {
        assert_eq!(recolte(5, 1.20), 6);
        assert_eq!(recolte(5, 1.35), 7);
    }
}

// ─── L'anneau d'apparition ne déborde plus l'enceinte (2026-08-04) ───────────

#[cfg(test)]
mod arena_clamp_tests {
    use super::*;

    /// Le défaut rapporté : collé au rempart, l'anneau du sniper (50 m) sort de
    /// l'arène et le mob naît DERRIÈRE le mur.
    #[test]
    fn a_spawn_beyond_the_ramparts_is_pulled_back_inside() {
        let arena = 80.0;
        // Joueur collé au mur (x = 78), anneau sniper 50 m vers l'extérieur.
        let voulu = Vec2::new(128.0, 0.0);
        let pose = clamp_into_arena(voulu, arena, 0.4);
        assert!(pose.length() <= arena, "dans l'enceinte : {}", pose.length());
        assert!(pose.length() > 0.0, "et pas ramené au centre");
    }

    /// La DIRECTION est conservée : l'ennemi arrive d'où l'anneau le voulait,
    /// simplement plus près. Le ramener au centre le ferait surgir dans le dos.
    #[test]
    fn the_direction_is_preserved_only_the_distance_shrinks() {
        let voulu = Vec2::new(60.0, 60.0);
        let pose = clamp_into_arena(voulu, 50.0, 0.4);
        let a = voulu.normalize();
        let b = pose.normalize();
        assert!((a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4, "{a:?} vs {b:?}");
    }

    /// Un point DÉJÀ dedans n'est pas touché : la borne ne doit pas resserrer
    /// l'anneau en temps normal, seulement l'empêcher de sortir.
    #[test]
    fn a_spawn_already_inside_is_left_alone() {
        let p = Vec2::new(10.0, -5.0);
        assert_eq!(clamp_into_arena(p, 80.0, 0.4), p);
    }

    /// Le RAYON du corps compte : un bot doit tenir entièrement dedans, pas
    /// affleurer le mur. Ignorer le rayon était le défaut d'origine de
    /// `spawn-clearance`.
    #[test]
    fn the_body_radius_is_part_of_the_bound() {
        let gros = clamp_into_arena(Vec2::new(200.0, 0.0), 50.0, 5.0);
        let petit = clamp_into_arena(Vec2::new(200.0, 0.0), 50.0, 0.4);
        assert!(gros.length() < petit.length(), "le gros s'arrête plus tôt");
        assert!(gros.length() <= 45.0 + 1e-3);
    }

    /// Enceinte inconnue (arène pas encore bâtie) → aucune borne. Le
    /// comportement d'avant, exactement — jamais un spawn au centre par défaut.
    #[test]
    fn an_unknown_arena_never_moves_anything() {
        let p = Vec2::new(999.0, -999.0);
        assert_eq!(clamp_into_arena(p, 0.0, 0.4), p);
    }
}

#[cfg(test)]
mod arena_hex_tests {
    use super::*;

    /// Le défaut RESTANT après le premier correctif : borner au cercle
    /// circonscrit laisse une couronne où l'on naît derrière le mur.
    ///
    /// Remparts hexagonaux inscrits → les murs sont à l'APOTHÈME (0,866 R), pas
    /// au rayon. Un point à 0,95 R en face d'une arête est dehors.
    #[test]
    fn the_ring_stops_at_the_wall_not_at_the_circumscribed_circle() {
        let r = 80.0;
        let apotheme = r * forgia_stage::layout::HEX_INSCRIBED_RATIO;
        let dans_la_couronne = Vec2::new(r * 0.95, 0.0);
        let pose = clamp_into_arena(dans_la_couronne, r, 0.4);
        assert!(
            pose.length() <= apotheme,
            "posé à {} alors que le mur est à {apotheme}",
            pose.length()
        );
    }

    /// …et on ne resserre pas pour autant l'anneau en temps normal : un spawn
    /// bien à l'intérieur reste intact.
    #[test]
    fn a_spawn_well_inside_is_still_untouched() {
        let p = Vec2::new(20.0, 10.0);
        assert_eq!(clamp_into_arena(p, 80.0, 0.4), p);
    }
}
