//! Barks des armes parlantes — story-531 AC5-AC7 (incrément 1 : event `kill`).
//!
//! Consomme ENFIN `assets/genomes/roguelite/roguelite_dialogue.toml` (écrit
//! story-472, dormant depuis l'abandon du refactor 471-479) : pools de répliques
//! pondérées par persona, pattern Hadès (weights + cooldown par ligne + lock
//! global anti-overlap + plafond barks/minute anti-fatigue).
//!
//! Trigger : `CombatHitEvent { is_kill: true, weapon }` (lecteur MULTICAST —
//! le killfeed lit le même message, aucun conflit). L'arme qui tue parle.
//! Affichage : bulle cartoon ancrée bas-droite (au-dessus des munitions),
//! liseré couleur persona, fade-out. Texte uniquement (armes parlantes MVP).

use std::collections::{HashMap, VecDeque};
use std::fs;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;

use crate::style::{forge_persona_color, FORGE_CREME};

const SENSOR_PATH: &str = "forgia2_barks.json";
const BARK_EVENT_KILL: &str = "kill";
/// Durée d'affichage : base + lecture (~12 chars/s), bornée.
const BARK_BASE_SECS: f32 = 1.6;
const BARK_PER_CHAR_SECS: f32 = 0.045;
const BARK_MAX_SECS: f32 = 4.0;
const BARK_FADE_SECS: f32 = 0.4;

/// Arme → clé `speaker` du genome (mapping persona dupliqué ui-lib,
/// cf confidence.rs / forgia-mode-roguelite audio.rs `weapon_fire_def`).
pub(crate) fn speaker_for(weapon: WeaponType) -> Option<&'static str> {
    match weapon {
        WeaponType::ModernAR => Some("pepin"),
        WeaponType::AssaultRifle => Some("bourrasque"),
        WeaponType::Shotgun => Some("lenoir"),
        WeaponType::RocketLauncher => Some("boucherie"),
        _ => None,
    }
}

/// Nom affiché dans la bulle.
fn display_name(speaker: &str) -> &'static str {
    match speaker {
        "pepin" => "Pépin",
        "bourrasque" => "Bourrasque",
        "lenoir" => "Mme Lenoir",
        "boucherie" => "Boucherie",
        _ => "???",
    }
}

// ─── Genome (couche definition — roguelite_dialogue.toml) ────────────────────

#[derive(Deserialize, Clone, Debug, Default)]
pub struct BarkGene {
    pub id: String,
    /// Valeur courante du gène (convention registry : `default` = valeur).
    #[serde(rename = "default")]
    pub value: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BarkLine {
    pub id: String,
    pub text: String,
    pub weight: u32,
    #[allow(dead_code)] // priority override = incréments suivants (death/boss).
    pub priority: u32,
    pub cooldown_sec: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BarkPool {
    pub speaker: String,
    pub event: String,
    pub lines: Vec<BarkLine>,
}

/// Désérialisation du TOML complet — les clés inconnues (id, name, domain,
/// label, chromosome, min/max…) sont ignorées par serde.
#[derive(Deserialize, TypePath, Clone, Debug, Default)]
#[serde(default)]
pub struct BarkGenome {
    pub genes: Vec<BarkGene>,
    pub pool: Vec<BarkPool>,
}

/// Tuning extrait des genes (défauts = miroir du TOML, zéro régression si
/// gène absent).
#[derive(Clone, Debug)]
pub struct BarkTuning {
    pub chance_on_kill: f32,
    pub speaker_lock_sec: f32,
    pub max_per_minute: usize,
}

impl Default for BarkTuning {
    fn default() -> Self {
        Self {
            chance_on_kill: 0.30,
            speaker_lock_sec: 2.5,
            max_per_minute: 12,
        }
    }
}

pub fn tuning_from(genes: &[BarkGene]) -> BarkTuning {
    let get = |id: &str, fallback: f32| {
        genes
            .iter()
            .find(|g| g.id == id)
            .map_or(fallback, |g| g.value)
    };
    let d = BarkTuning::default();
    BarkTuning {
        chance_on_kill: get("bark_chance_on_kill", d.chance_on_kill),
        speaker_lock_sec: get("bark_global_speaker_lock_sec", d.speaker_lock_sec),
        max_per_minute: get("bark_global_max_per_minute", d.max_per_minute as f32) as usize,
    }
}

/// Bibliothèque runtime synced depuis l'asset genome (hot-reload file_watcher).
#[derive(Resource, Default)]
pub struct BarkLibrary {
    pub pools: Vec<BarkPool>,
    pub tuning: BarkTuning,
}

#[derive(Resource, Default)]
pub struct BarkTuningHandleState(pub Option<Handle<Genome<BarkGenome>>>);

fn load_bark_genome(mut state: ResMut<BarkTuningHandleState>, asset_server: Res<AssetServer>) {
    state.0 = Some(asset_server.load("genomes/roguelite/roguelite_dialogue.toml"));
}

fn sync_bark_library(
    state: Res<BarkTuningHandleState>,
    assets: Res<Assets<Genome<BarkGenome>>>,
    mut library: ResMut<BarkLibrary>,
) {
    let Some(g) = state.0.as_ref().and_then(|h| assets.get(h)) else {
        return;
    };
    // Sync à chaque frame est inutile : ne recopier que si le contenu a bougé
    // (taille pools = proxy suffisant, hot-reload recharge l'asset entier).
    if library.pools.len() == g.data.pool.len() && !library.pools.is_empty() {
        return;
    }
    library.pools = g.data.pool.clone();
    library.tuning = tuning_from(&g.data.genes);
    info!(
        "[barks] library loaded: {} pools, P(kill)={:.2}, lock={}s, max/min={}",
        library.pools.len(),
        library.tuning.chance_on_kill,
        library.tuning.speaker_lock_sec,
        library.tuning.max_per_minute
    );
}

// ─── Moteur (sélection pure + état) ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ActiveBark {
    pub speaker: String,
    pub text: String,
    pub until: f32,
}

#[derive(Resource)]
pub struct BarkEngine {
    pub active: Option<ActiveBark>,
    /// line id → instant (elapsed secs) où la ligne redevient éligible.
    pub line_ready_at: HashMap<String, f32>,
    /// Anti-overlap : aucun bark avant cet instant (lock global, simplif. du
    /// speaker_lock — le kill est priority 30, pas d'override death ici).
    pub lock_until: f32,
    /// Timestamps des barks joués (fenêtre 60 s pour le plafond par minute).
    pub recent: VecDeque<f32>,
    pub rng: u64,
    pub played_total: u32,
    pub suppressed_lock: u32,
    pub suppressed_rate: u32,
    pub last_line_id: String,
}

impl Default for BarkEngine {
    fn default() -> Self {
        Self {
            active: None,
            line_ready_at: HashMap::new(),
            lock_until: 0.0,
            recent: VecDeque::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            played_total: 0,
            suppressed_lock: 0,
            suppressed_rate: 0,
            last_line_id: String::new(),
        }
    }
}

/// xorshift64 — aléa léger sans dépendance, déterministe en test.
pub(crate) fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x.max(1); // jamais 0 (point fixe xorshift)
    x
}

/// u64 → [0,1).
pub(crate) fn roll01(state: &mut u64) -> f32 {
    (xorshift(state) >> 40) as f32 / (1u64 << 24) as f32
}

#[derive(Debug, PartialEq, Eq)]
pub enum BarkGate {
    Allowed,
    Locked,
    RateLimited,
}

/// Portail anti-spam — pur, testable. `recent_in_window` = barks joués dans
/// les 60 dernières secondes (déjà élagués par l'appelant).
pub(crate) fn gate(
    now: f32,
    lock_until: f32,
    recent_in_window: usize,
    max_per_minute: usize,
) -> BarkGate {
    if now < lock_until {
        BarkGate::Locked
    } else if recent_in_window >= max_per_minute {
        BarkGate::RateLimited
    } else {
        BarkGate::Allowed
    }
}

/// Tirage pondéré parmi les lignes éligibles. `roll01` ∈ [0,1).
pub(crate) fn pick_weighted<'a>(candidates: &[&'a BarkLine], roll01: f32) -> Option<&'a BarkLine> {
    let total: u32 = candidates.iter().map(|l| l.weight).sum();
    if total == 0 {
        return None;
    }
    let mut target = (roll01 * total as f32) as u32;
    for line in candidates {
        if target < line.weight {
            return Some(line);
        }
        target -= line.weight;
    }
    candidates.last().copied()
}

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Lit les kills, fait parler l'arme qui tue. Tourne aussi hors-combat pour
/// expirer le bark actif (cheap : early-return sans events).
fn sys_trigger_kill_barks(
    mut hits: MessageReader<CombatHitEvent>,
    mut engine: ResMut<BarkEngine>,
    library: Res<BarkLibrary>,
    time: Res<Time>,
    game_mode: Res<State<GameMode>>,
) {
    let now = time.elapsed_secs();
    if let Some(active) = &engine.active {
        if now >= active.until {
            engine.active = None;
        }
    }
    if *game_mode.get() != GameMode::Roguelite {
        hits.clear();
        return;
    }
    // Mélange l'horloge dans le state rng (varie les runs sans Date/rand).
    engine.rng ^= time.elapsed().subsec_nanos() as u64 | 1;

    for hit in hits.read() {
        if !hit.is_kill {
            continue;
        }
        let Some(speaker) = hit.weapon.and_then(speaker_for) else {
            continue;
        };
        // P(bark) — un kill silencieux est normal (anti-fatigue, Hadès).
        if roll01(&mut engine.rng) >= library.tuning.chance_on_kill {
            continue;
        }
        engine.recent.retain(|t| now - t < 60.0);
        match gate(
            now,
            engine.lock_until,
            engine.recent.len(),
            library.tuning.max_per_minute,
        ) {
            BarkGate::Locked => {
                engine.suppressed_lock += 1;
                continue;
            }
            BarkGate::RateLimited => {
                engine.suppressed_rate += 1;
                continue;
            }
            BarkGate::Allowed => {}
        }
        let Some(pool) = library
            .pools
            .iter()
            .find(|p| p.speaker == speaker && p.event == BARK_EVENT_KILL)
        else {
            continue;
        };
        let candidates: Vec<&BarkLine> = pool
            .lines
            .iter()
            .filter(|l| engine.line_ready_at.get(&l.id).is_none_or(|&t| now >= t))
            .collect();
        let roll = roll01(&mut engine.rng);
        let Some(line) = pick_weighted(&candidates, roll) else {
            continue; // tout le pool en cooldown
        };
        let duration = (BARK_BASE_SECS + line.text.chars().count() as f32 * BARK_PER_CHAR_SECS)
            .min(BARK_MAX_SECS);
        let line_id = line.id.clone();
        let text = line.text.clone();
        let ready_at = now + line.cooldown_sec;
        engine.line_ready_at.insert(line_id.clone(), ready_at);
        engine.lock_until = now + library.tuning.speaker_lock_sec;
        engine.recent.push_back(now);
        engine.played_total += 1;
        engine.last_line_id = line_id;
        engine.active = Some(ActiveBark {
            speaker: speaker.to_string(),
            text,
            until: now + duration,
        });
    }
}

/// Bulle cartoon bas-droite (l'arme parle au-dessus du compteur munitions).
fn sys_draw_bark(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    engine: Res<BarkEngine>,
    time: Res<Time>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Some(active) = &engine.active else { return };
    let now = time.elapsed_secs();
    if now >= active.until {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Fade-out sur la fin de vie (gamma_multiply ≈ alpha perceptuel).
    let remaining = active.until - now;
    let fade = (remaining / BARK_FADE_SECS).clamp(0.0, 1.0);
    let persona = forge_persona_color(&active.speaker).gamma_multiply(fade);
    let fill = egui::Color32::from_rgba_unmultiplied(24, 20, 18, 230).gamma_multiply(fade);
    let text_color = FORGE_CREME.gamma_multiply(fade);

    egui::Area::new(egui::Id::new("forgia_weapon_bark"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -170.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(fill)
                .stroke(egui::Stroke::new(3.0, persona))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_max_width(280.0);
                    ui.label(
                        egui::RichText::new(display_name(&active.speaker))
                            .size(13.0)
                            .strong()
                            .color(persona),
                    );
                    ui.label(
                        egui::RichText::new(&active.text)
                            .size(16.0)
                            .strong()
                            .color(text_color),
                    );
                });
        });
}

/// Sensor `forgia2_barks.json` 1Hz — observabilité obligatoire.
fn sys_write_barks_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    engine: Res<BarkEngine>,
    library: Res<BarkLibrary>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = if library.pools.is_empty() {
        (
            "info",
            "0 pool charge - verifier assets/genomes/roguelite/roguelite_dialogue.toml + loader Genome<BarkGenome>",
        )
    } else {
        ("ok", "")
    };
    let active = engine
        .active
        .as_ref()
        .map(|a| a.text.as_str())
        .unwrap_or("");
    let json = format!(
        r#"{{"id":"barks","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"pools_loaded":{},"chance_on_kill":{:.2},"played_total":{},"suppressed_lock":{},"suppressed_rate":{},"last_line_id":"{}","active_text":"{}"}}"#,
        time.elapsed_secs(),
        library.pools.len(),
        library.tuning.chance_on_kill,
        engine.played_total,
        engine.suppressed_lock,
        engine.suppressed_rate,
        engine.last_line_id,
        json_escape(active),
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[barks] sensor write failed: {e}");
    }
}

/// Échappement JSON minimal (les répliques ne contiennent ni `\n` ni contrôle).
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct WeaponBarksPlugin;

impl Plugin for WeaponBarksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BarkEngine>()
            .init_resource::<BarkLibrary>()
            .init_resource::<BarkTuningHandleState>()
            .init_asset::<Genome<BarkGenome>>()
            .register_asset_loader(GenomeLoader::<BarkGenome>::default())
            .add_systems(Startup, load_bark_genome)
            .add_systems(
                Update,
                (sync_bark_library, sys_trigger_kill_barks)
                    .chain()
                    .in_set(GameSet::Effects),
            )
            .add_systems(Update, sys_write_barks_sensor.in_set(GameSet::Sensors))
            .add_systems(EguiPrimaryContextPass, sys_draw_bark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_damage::HitZone;

    #[test]
    fn genome_parses_real_toml_shape() {
        let src = r#"
id = "roguelite_dialogue"
name = "Roguelite Talking Weapons Dialogue"

[[genes]]
id = "bark_chance_on_kill"
label = "P(bark) on enemy kill"
chromosome = "Trigger"
min = 0.0
max = 1.0
default = 0.30
layer = "roguelite_dialogue"

[[pool]]
speaker = "pepin"
event = "kill"
lines = [
  { id = "pepin_kill_01", text = "Oh... pardon...", weight = 30, priority = 30, cooldown_sec = 12.0 },
]
"#;
        let g: BarkGenome = forgia_genome_core::parse_genome(src).expect("parse");
        assert_eq!(g.pool.len(), 1);
        assert_eq!(g.pool[0].lines[0].text, "Oh... pardon...");
        assert_eq!(g.pool[0].lines[0].weight, 30);
        let t = tuning_from(&g.genes);
        assert!((t.chance_on_kill - 0.30).abs() < 1e-6);
        // Gènes absents → défauts miroir.
        assert!((t.speaker_lock_sec - 2.5).abs() < 1e-6);
        assert_eq!(t.max_per_minute, 12);
    }

    #[test]
    fn pick_weighted_is_deterministic_and_bounded() {
        let mk = |id: &str, w: u32| BarkLine {
            id: id.into(),
            text: id.into(),
            weight: w,
            priority: 30,
            cooldown_sec: 10.0,
        };
        let lines = [mk("a", 30), mk("b", 25), mk("c", 20), mk("d", 25)];
        let refs: Vec<&BarkLine> = lines.iter().collect();
        assert_eq!(pick_weighted(&refs, 0.0).unwrap().id, "a");
        assert_eq!(pick_weighted(&refs, 0.999).unwrap().id, "d");
        // 0.30 → cumulé : a=30 (0.30*100=30 → ≥30 passe), b couvre [30,55).
        assert_eq!(pick_weighted(&refs, 0.31).unwrap().id, "b");
        assert!(pick_weighted(&[], 0.5).is_none());
    }

    #[test]
    fn gate_lock_then_rate_then_allowed() {
        assert_eq!(gate(1.0, 2.5, 0, 12), BarkGate::Locked);
        assert_eq!(gate(3.0, 2.5, 12, 12), BarkGate::RateLimited);
        assert_eq!(gate(3.0, 2.5, 3, 12), BarkGate::Allowed);
    }

    #[test]
    fn speaker_mapping_covers_arena_v1_only() {
        assert_eq!(speaker_for(WeaponType::ModernAR), Some("pepin"));
        assert_eq!(speaker_for(WeaponType::AssaultRifle), Some("bourrasque"));
        assert_eq!(speaker_for(WeaponType::Shotgun), Some("lenoir"));
        assert_eq!(speaker_for(WeaponType::RocketLauncher), Some("boucherie"));
        assert_eq!(speaker_for(WeaponType::AK47), None);
    }

    /// Headless : un kill Pépin déclenche un bark ; le 2e kill de la même frame
    /// est bloqué par le lock anti-overlap.
    #[test]
    fn kill_event_triggers_bark_once_per_lock_window() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.insert_state(GameMode::Roguelite);
        app.init_resource::<BarkEngine>();
        app.insert_resource(BarkLibrary {
            pools: vec![BarkPool {
                speaker: "pepin".into(),
                event: "kill".into(),
                lines: vec![BarkLine {
                    id: "pepin_kill_01".into(),
                    text: "Oh... pardon...".into(),
                    weight: 100,
                    priority: 30,
                    cooldown_sec: 12.0,
                }],
            }],
            tuning: BarkTuning {
                chance_on_kill: 1.0, // déterministe pour le test
                ..Default::default()
            },
        });
        app.init_resource::<Time>();
        app.add_message::<CombatHitEvent>();
        app.add_systems(Update, sys_trigger_kill_barks);

        for _ in 0..2 {
            app.world_mut().write_message(CombatHitEvent {
                target: Entity::PLACEHOLDER,
                attacker: None,
                damage: 40.0,
                is_kill: true,
                is_headshot: false,
                hit_world_pos: Vec3::ZERO,
                weapon: Some(WeaponType::ModernAR),
                body_zone: HitZone::Body,
            });
        }
        app.update();

        let engine = app.world().resource::<BarkEngine>();
        assert_eq!(engine.played_total, 1, "1er kill parle");
        assert_eq!(engine.suppressed_lock, 1, "2e kill même frame = lock");
        let active = engine.active.as_ref().expect("bulle active");
        assert_eq!(active.text, "Oh... pardon...");
        assert_eq!(engine.last_line_id, "pepin_kill_01");
    }
}
