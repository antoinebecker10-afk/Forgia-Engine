//! weapon_select.rs — Story-612 Phase 0. Wizard de choix d'arme de départ.
//!
//! Premier choix signifiant de pré-run : au Lobby (L'Enclume des Âmes), le joueur
//! choisit son arme de départ parmi les 4 (← / →) au lieu de démarrer d'office
//! avec Pépin. La carte affiche les **vraies** stats de combat + l'élément
//! signature + le matchup « fort vs / faible vs ».
//!
//! ## Source de vérité (concept-first, story-612)
//!
//! Les stats viennent de `assets/genomes/viewmodel_arena.toml` (la VRAIE source
//! lue par `forgia-fps` via `ViewmodelGenomeEntry`), PAS de `roguelite_weapons.toml`
//! (genome mort, 0 consommateur, valeurs divergentes). Lecture directe fs + mtime
//! hot-reload — même pattern que `meta_shop.rs` et `elements.rs` dans cette crate
//! (zéro nouvelle dépendance cross-crate). Le `Default` est le miroir exact du TOML.
//!
//! Piège de nommage (enum legacy V1) : `WeaponType::Shotgun` = Madame Lenoir
//! (sniper), `WeaponType::RocketLauncher` = Boucherie. Clé genome via [`vm_key`].

use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::weapons::{EquippedWeapons, WeaponType, ARENA_V1_WEAPONS};
use forgia_core::prelude::*;
use forgia_ui_lib::style::{
    C_HP_HIGH, C_TEXT_LIGHT, C_TEXT_MUTED, FORGE_OR, FORGE_PANEL, FORGE_TEAL, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use crate::element_vfx::ElementVfxAssets;
use crate::elements::{Element, ElementConfig};
use crate::enemies::EnemyArchetype;
use crate::meta_shop::{MetaShopCatalogue, MetaShopSave};
use crate::run::{weapon_to_speaker, MetaSouls, RunState};

const VIEWMODEL_GENOME_PATH: &str = "assets/genomes/viewmodel_arena.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── État du choix ──────────────────────────────────────────────────────────

/// Index de l'arme de départ choisie dans [`ARENA_V1_WEAPONS`]. Default 0 = Pépin
/// (= ancien comportement `EquippedWeapons::default()`) → backward-compatible.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct StartingWeaponChoice {
    pub idx: usize,
}

// ─── Carte de stats (sous-ensemble gameplay de ViewmodelGenomeEntry) ─────────

/// Champs gameplay lus pour la carte. Les autres champs du TOML (offsets, ADS,
/// juice…) sont ignorés par serde (pas de `deny_unknown_fields`).
#[derive(Deserialize, Clone, Debug)]
pub struct WeaponCard {
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub fire_rate: f32,
    #[serde(default)]
    pub range: f32,
    #[serde(default = "one_u32")]
    pub pellets: u32,
    #[serde(default)]
    pub mag_size: u32,
    #[serde(default)]
    pub reload_time_secs: f32,
    #[serde(default = "one_f32")]
    pub head_damage_mul: f32,
}

fn one_u32() -> u32 {
    1
}
fn one_f32() -> f32 {
    1.0
}

#[derive(Deserialize, Default)]
struct WeaponCardTable {
    #[serde(default)]
    weapons: HashMap<String, WeaponCard>,
}

/// Cartes par arme + mtime pour le hot-reload (miroir du pattern `ElementGenomeWatch`).
#[derive(Resource, Default)]
pub struct WeaponCards {
    cards: HashMap<WeaponType, WeaponCard>,
    last_mtime: Option<SystemTime>,
}

/// Map `WeaponType` (enum legacy) → clé TOML `[weapons.<key>]` de `viewmodel_arena.toml`.
/// Miroir de `forgia_viewmodel::weapon_genome_key` (dupliqué pour éviter la dép crate).
pub fn vm_key(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "pepin",
        WeaponType::AssaultRifle => "bourrasque",
        WeaponType::Shotgun => "madame_lenoir",
        WeaponType::RocketLauncher => "boucherie",
        _ => "pepin",
    }
}

/// DPS soutenu = dégâts × cadence (tirs/s) × pellets. Pur, testable.
/// Cas roquette (`damage = 0`, Boucherie) → 0 ; l'appelant étiquette « AOE ».
pub fn weapon_dps(damage: f32, fire_rate: f32, pellets: u32) -> f32 {
    damage * fire_rate * pellets.max(1) as f32
}

/// Chemin du GLB d'arme (relatif à `assets/`) pour l'aperçu 3D du hub-menu (RTT).
/// `idx` = index dans [`ARENA_V1_WEAPONS`] (= `StartingWeaponChoice.idx`). Évite
/// d'exposer `vm_key`/`ARENA_V1_WEAPONS` à `forgia-ui`.
pub fn weapon_preview_glb_path(idx: usize) -> String {
    let w = ARENA_V1_WEAPONS[idx % ARENA_V1_WEAPONS.len()];
    format!("models/weapons/forgia/{}.glb", vm_key(w))
}

/// Miroir EXACT des 4 armes de `viewmodel_arena.toml` (fallback si fichier KO).
fn mirror_default() -> HashMap<WeaponType, WeaponCard> {
    HashMap::from([
        (
            WeaponType::ModernAR,
            WeaponCard {
                damage: 28.0,
                fire_rate: 6.0,
                range: 80.0,
                pellets: 1,
                mag_size: 12,
                reload_time_secs: 1.2,
                head_damage_mul: 2.0,
            },
        ),
        (
            WeaponType::AssaultRifle,
            WeaponCard {
                damage: 11.0,
                fire_rate: 11.0,
                range: 30.0,
                pellets: 1,
                mag_size: 30,
                reload_time_secs: 1.6,
                head_damage_mul: 1.5,
            },
        ),
        (
            WeaponType::Shotgun,
            WeaponCard {
                damage: 50.0,
                fire_rate: 0.8,
                range: 300.0,
                pellets: 1,
                mag_size: 5,
                reload_time_secs: 2.5,
                head_damage_mul: 2.0,
            },
        ),
        (
            WeaponType::RocketLauncher,
            WeaponCard {
                damage: 0.0,
                fire_rate: 0.9,
                range: 60.0,
                pellets: 1,
                mag_size: 3,
                reload_time_secs: 1.33,
                head_damage_mul: 1.0,
            },
        ),
    ])
}

/// Pur — testable. Garde uniquement les 4 armes Arena V1. Fallback miroir si vide.
fn parse_cards(content: &str) -> HashMap<WeaponType, WeaponCard> {
    let table: WeaponCardTable = toml::from_str(content).unwrap_or_default();
    let mut out = HashMap::new();
    for w in ARENA_V1_WEAPONS {
        if let Some(c) = table.weapons.get(vm_key(w)) {
            out.insert(w, c.clone());
        }
    }
    if out.is_empty() {
        mirror_default()
    } else {
        out
    }
}

fn load_cards() -> HashMap<WeaponType, WeaponCard> {
    match fs::read_to_string(VIEWMODEL_GENOME_PATH) {
        Ok(content) => parse_cards(&content),
        Err(_) => mirror_default(),
    }
}

// ─── Identité persona + libellés ─────────────────────────────────────────────

/// (Nom persona, tagline) — texte UI cosmétique (cf creator-simplicity).
fn persona(w: WeaponType) -> (&'static str, &'static str) {
    match w {
        WeaponType::ModernAR => ("Pépin", "Pistolet ricocheur"),
        WeaponType::AssaultRifle => ("Bourrasque", "Mitraillette du vent"),
        WeaponType::Shotgun => ("Madame Lenoir", "Sniper aristocrate"),
        WeaponType::RocketLauncher => ("Boucherie", "Lance-roquettes brutal"),
        _ => ("?", ""),
    }
}

fn arch_fr(a: EnemyArchetype) -> &'static str {
    match a {
        EnemyArchetype::Tank => "Tank",
        EnemyArchetype::Runner => "Coureur",
        EnemyArchetype::Sniper => "Tireur",
        EnemyArchetype::Boss => "Boss",
    }
}

/// Archétype le plus fort (max) et le plus faible (min) pour un élément donné.
fn strong_weak(cfg: &ElementConfig, e: Element) -> ((EnemyArchetype, f32), (EnemyArchetype, f32)) {
    let arches = [
        EnemyArchetype::Tank,
        EnemyArchetype::Runner,
        EnemyArchetype::Sniper,
        EnemyArchetype::Boss,
    ];
    let mut best = (arches[0], cfg.matchup_for(e, arches[0]));
    let mut worst = best;
    for &a in &arches[1..] {
        let m = cfg.matchup_for(e, a);
        if m > best.1 {
            best = (a, m);
        }
        if m < worst.1 {
            worst = (a, m);
        }
    }
    (best, worst)
}

/// Couleur du badge élément, dérivée des RGB linéaires du genome (gamma → sRGB).
fn elem_color(e: Element, cfg: &ElementConfig) -> egui::Color32 {
    let [r, g, b] = e.rgb(&cfg.vfx);
    let g8 = |c: f32| (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    egui::Color32::from_rgb(g8(r), g8(g), g8(b))
}

// ─── Systems ─────────────────────────────────────────────────────────────────

/// Startup — charge les cartes depuis `viewmodel_arena.toml` (+ mtime).
pub fn sys_load_weapon_cards(mut commands: Commands) {
    let cards = load_cards();
    let mtime = fs::metadata(VIEWMODEL_GENOME_PATH)
        .and_then(|m| m.modified())
        .ok();
    info!("[weapon-select] cartes chargées — {} armes", cards.len());
    commands.insert_resource(WeaponCards {
        cards,
        last_mtime: mtime,
    });
}

/// Poll mtime 1Hz → re-parse si le genome a changé (hot-reload Shift+F12-like).
pub fn sys_hot_reload_weapon_cards(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cards: ResMut<WeaponCards>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(meta) = fs::metadata(VIEWMODEL_GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if cards.last_mtime == Some(mtime) {
        return;
    }
    cards.cards = load_cards();
    cards.last_mtime = Some(mtime);
    info!("[weapon-select] cartes HOT-RELOADED");
}

/// Lobby : ← / → cyclent l'arme choisie. Layout-agnostic (AZERTY-safe), pas de
/// conflit avec les touches 1-4 / ENTRÉE du meta-shop.
pub fn sys_weapon_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut choice: ResMut<StartingWeaponChoice>,
) {
    let n = ARENA_V1_WEAPONS.len();
    if keys.just_pressed(KeyCode::ArrowRight) {
        choice.idx = (choice.idx + 1) % n;
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        choice.idx = (choice.idx + n - 1) % n;
    }
}

/// OnExit(Lobby) = run-start : applique le choix à `EquippedWeapons.current` + calcule
/// le bonus de maîtrise (niveau de l'arme) dans `WeaponMasteryMods`. No-op si la
/// Resource est absente.
///
/// Le bonus/niveau ET le plafond viennent du genome `[mastery]` (`MetaShopCatalogue`),
/// plus d'un `const` Rust : la maîtrise est de la balance, elle vit en couche definition.
pub fn sys_apply_weapon_choice(
    choice: Res<StartingWeaponChoice>,
    save: Res<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    equipped: Option<ResMut<EquippedWeapons>>,
    mut mastery: ResMut<crate::meta_shop::WeaponMasteryMods>,
) {
    let Some(mut eq) = equipped else {
        return;
    };
    let chosen = ARENA_V1_WEAPONS[choice.idx % ARENA_V1_WEAPONS.len()];
    // Sécurité (story-613) : jamais démarrer avec une arme verrouillée → fallback Pépin.
    let w = if save.is_weapon_unlocked(vm_key(chosen)) {
        chosen
    } else {
        WeaponType::ModernAR
    };
    eq.current = w;
    // 2026-08-04 — le bonus est le TOTAL, toutes armes confondues : il ne dépend
    // plus de l'arme choisie. `current` ne sert plus qu'à l'affichage.
    let level = save.weapon_level(vm_key(w));
    mastery.damage_mul = cat.mastery.total_damage_mul(&save.weapon_levels);
    mastery.current = w;
    info!(
        "[weapon-select] run start — arme = {:?} (niv {level}/{}, dmg ×{:.2})",
        w, cat.mastery.max_level, mastery.damage_mul
    );
}

/// Aligne la maîtrise sur l'arme **actuellement équipée**, chaque frame.
///
/// Sans ce système, le bonus restait figé sur l'arme choisie au run-start : changer
/// d'arme avec Digit1-4 gardait la maîtrise de l'autre. Mesuré en jeu le 2026-08-04
/// (cf. `WeaponMasteryMods`) — le joueur perdait la progression de l'arme qu'il
/// tenait réellement, ce qui décourage exactement le changement d'arme que le
/// matchup élémentaire est censé récompenser.
///
/// Miroir exact de `trempe::sys_sync_trempe_current` : même cadence, même coût
/// (une lecture + une comparaison), même lag ≤ 1 frame avant le recompute des mods.
pub fn sys_sync_mastery_current(
    save: Res<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut mastery: ResMut<crate::meta_shop::WeaponMasteryMods>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    let Some(eq) = equipped else { return };
    let w = eq.current;
    // Le total ne dépend PAS de `w` — c'est tout l'objet du changement. On le
    // recalcule quand même chaque frame (coût nul, ≤ 4 entrées) pour que le
    // niveau gagné en fin de run se voie sans redémarrer.
    let mul = cat.mastery.total_damage_mul(&save.weapon_levels);
    if mastery.current != w || (mastery.damage_mul - mul).abs() > 1e-6 {
        mastery.current = w;
        mastery.damage_mul = mul;
    }
}

/// En run, empêche d'UTILISER une arme verrouillée : le switch Digit2-4
/// (`forgia_fps::weapon_select_system`, GameSet::Combat) peut mettre `current` sur
/// une arme non débloquée. Ce garde tourne en `GameSet::Movement` (AVANT Combat/tir)
/// et ramène `current` sur la dernière arme débloquée connue (départ = Pépin).
/// Story-613 — gating cross-crate sans cycle (le save vit ici, pas dans forgia-fps).
fn sys_enforce_unlocked_loadout(
    save: Res<MetaShopSave>,
    equipped: Option<ResMut<EquippedWeapons>>,
    mut last_ok: Local<Option<WeaponType>>,
) {
    let Some(mut eq) = equipped else {
        return;
    };
    if save.is_weapon_unlocked(vm_key(eq.current)) {
        *last_ok = Some(eq.current);
        return;
    }
    let fallback = (*last_ok)
        .filter(|w| save.is_weapon_unlocked(vm_key(*w)))
        .unwrap_or(WeaponType::ModernAR);
    if eq.current != fallback {
        info!(
            "[weapon-select] arme verrouillée {:?} bloquée → {:?}",
            eq.current, fallback
        );
        eq.current = fallback;
    }
}

/// Lobby — [U] débloque l'arme sélectionnée si verrouillée + assez d'Âmes (story-613).
/// Mute MetaSouls + MetaShopSave (persisté disque), miroir de `meta_shop::sys_meta_shop_input`.
pub fn sys_weapon_unlock_input(
    keys: Res<ButtonInput<KeyCode>>,
    choice: Res<StartingWeaponChoice>,
    cat: Res<MetaShopCatalogue>,
    mut save: ResMut<MetaShopSave>,
    mut meta: ResMut<MetaSouls>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    let w = ARENA_V1_WEAPONS[choice.idx % ARENA_V1_WEAPONS.len()];
    let key = vm_key(w);
    if save.is_weapon_unlocked(key) {
        return; // déjà possédée
    }
    let Some(unlock) = cat.weapon_unlock(key) else {
        return; // pas déblocable (Pépin / inconnue)
    };
    if meta.current < unlock.cost {
        info!(
            "[weapon-select] pas assez d'Âmes pour {} ({}/{})",
            unlock.name, meta.current, unlock.cost
        );
        return;
    }
    meta.current -= unlock.cost;
    save.unlock_weapon(key);
    save.souls_total = meta.current;
    save.save();
    info!(
        "[weapon-select] ARME DÉBLOQUÉE : {} (-{} Âmes, reste {})",
        unlock.name, unlock.cost, meta.current
    );
}

/// Run terminée (Defeat/Victory) → l'arme équipée gagne 1 niveau de maîtrise (P3,
/// persisté), dans la limite du plafond `[mastery] max_level` du genome. Chaque arme
/// progresse indépendamment selon son usage.
pub fn sys_level_up_equipped_weapon(
    equipped: Option<Res<EquippedWeapons>>,
    cat: Res<MetaShopCatalogue>,
    mut save: ResMut<MetaShopSave>,
) {
    let Some(eq) = equipped else {
        return;
    };
    let key = vm_key(eq.current);
    let before = save.weapon_level(key);
    let level = save.level_up_weapon(key, cat.mastery.max_level);
    if level == before {
        return; // déjà au plafond : pas de niveau gagné, pas d'écriture disque
    }
    save.save();
    info!(
        "[weapon-select] {key} → niveau {level}/{}",
        cat.mastery.max_level
    );
}

/// Dessine la carte de choix d'arme au Lobby (à droite de L'Enclume).
pub fn draw_weapon_select(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    mut choice: ResMut<StartingWeaponChoice>,
    cards: Res<WeaponCards>,
    elem_cfg: Res<ElementConfig>,
    mut save: ResMut<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut meta: ResMut<MetaSouls>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !matches!(run_state.as_deref().map(|s| s.get()), Some(RunState::Lobby)) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Story-617 — scrim sombre plein écran : recule l'arène 3D live derrière l'UI
    // → l'écran « se referme » sur la sélection. Order::Background = derrière les panneaux.
    {
        let screen = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("ws_scrim"),
        ))
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
    }

    let n = ARENA_V1_WEAPONS.len();
    let sel = choice.idx % n;
    let w = ARENA_V1_WEAPONS[sel];
    let key = vm_key(w);
    let owned = save.is_weapon_unlocked(key);
    let unlock = cat.weapon_unlock(key);
    // Carte grisée si verrouillée (accent muted) → signal visuel "locked".
    let accent = if owned {
        crate::hud::speaker_color(weapon_to_speaker(w))
    } else {
        C_TEXT_MUTED
    };
    let (name, tagline) = persona(w);
    let card = cards.cards.get(&w);
    let element = elem_cfg.element_for(w);

    // ── Carte centrale (vitrine d'arme) : l'aperçu 3D (preview agrandi) flotte AU-DESSUS,
    // la carte stats + sélecteur ◄ ► est centrée juste dessous. Le titre « CHOISIS TON
    // ARME » est fourni par le hub (onglet) ; le bas-centre est libre pour LANCER (hub).
    // Home-hub P2.1 : avant = 3 zones (titre haut / stats gauche / sélecteur bas).
    egui::Area::new(egui::Id::new("ws_card"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Cadre EXTÉRIEUR transparent + bordure : son HAUT est un viewport vide
            // (transparent) où l'aperçu 3D (layer 0, rendu derrière egui) apparaît →
            // l'arme est "dans" la carte. Le panneau stats (fill opaque) est dessous.
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin::same(6))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(3.0, accent))
                .show(ui, |ui| {
                    ui.set_min_width(420.0);
                    ui.set_max_width(460.0);
                    ui.vertical_centered(|ui| {
                        // Viewport d'arme : espace vide (l'aperçu 3D rend derrière egui).
                        ui.add_space(WEAPON_VIEWPORT_H);
                        egui::Frame::new()
                            .fill(FORGE_PANEL)
                            .inner_margin(egui::Margin::symmetric(24, 16))
                            .corner_radius(egui::CornerRadius::same(10))
                            .stroke(egui::Stroke::new(1.5, HAIR_GOLD_STRONG))
                            .show(ui, |ui| {
                                ui.set_min_width(380.0);
                                // Taille UNIFIÉE : hauteur stats fixe → carte identique
                                // pour toutes les armes (design cohérent + viewport 3D
                                // stable, ne bouge plus selon l'arme).
                                ui.set_min_height(STATS_PANEL_H);
                                ui.vertical(|ui| {
                                    // En-tête : nom + index parcouru + tagline.
                                    ui.horizontal(|ui| {
                                        ui.heading(display_text(name, 28.0, accent).strong());
                                        ui.label(
                                            egui::RichText::new(format!("‹ {}/{} ›", sel + 1, n))
                                                .size(16.0)
                                                .color(FORGE_TEAL),
                                        );
                                    });
                                    ui.label(
                                        egui::RichText::new(tagline)
                                            .size(14.0)
                                            .italics()
                                            .color(C_TEXT_MUTED),
                                    );
                                    // Statut verrou + NIVEAU de maîtrise (P3) — visible par arme.
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        if owned {
                                            ui.label(
                                                egui::RichText::new("DÉBLOQUÉE")
                                                    .size(15.0)
                                                    .strong()
                                                    .color(C_HP_HIGH),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new("VERROUILLÉE")
                                                    .size(15.0)
                                                    .strong()
                                                    .color(C_TEXT_MUTED),
                                            );
                                        }
                                        // Niveau EFFECTIF : un save antérieur au plafond
                                        // stocke 13 ; afficher « 13/6 » serait illisible.
                                        // Bonus dérivé de la même source que le runtime.
                                        let lvl = cat.mastery.effective_level(save.weapon_level(key));
                                        let bonus = (cat.mastery.damage_mul(lvl) - 1.0) * 100.0;
                                        let cap = cat.mastery.max_level;
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "·  Niveau {lvl}/{cap}  (+{bonus:.0}% dégâts)"
                                            ))
                                            .size(15.0)
                                            .strong()
                                            .color(FORGE_OR),
                                        );
                                    });
                                    ui.add_space(8.0);

                                    if let Some(e) = element {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Élément : {} — {}",
                                                e.fr_name(),
                                                e.tag()
                                            ))
                                            .size(16.0)
                                            .strong()
                                            .color(elem_color(e, &elem_cfg)),
                                        );
                                        ui.add_space(8.0);
                                    }

                                    match card {
                                        Some(c) => {
                                            let dps = if c.damage <= 0.0 {
                                                "roquette AOE".to_string()
                                            } else {
                                                format!(
                                                    "{:.0}",
                                                    weapon_dps(c.damage, c.fire_rate, c.pellets)
                                                )
                                            };
                                            let dmg = if c.damage <= 0.0 {
                                                "—".to_string()
                                            } else {
                                                format!("{:.0}", c.damage)
                                            };
                                            let mag = c.mag_size.to_string();
                                            egui::Grid::new("forgia_ws_grid")
                                                .num_columns(2)
                                                .spacing([18.0, 5.0])
                                                .show(ui, |ui| {
                                                    stat_row(ui, "DMG / coup", &dmg);
                                                    stat_row(
                                                        ui,
                                                        "Cadence",
                                                        &format!("{:.1} /s", c.fire_rate),
                                                    );
                                                    stat_row_strong(ui, "DPS", &dps, FORGE_OR);
                                                    stat_row(ui, "Chargeur", &mag);
                                                    stat_row(
                                                        ui,
                                                        "Recharge",
                                                        &format!("{:.2} s", c.reload_time_secs),
                                                    );
                                                    stat_row(
                                                        ui,
                                                        "Portée",
                                                        &format!("{:.0} m", c.range),
                                                    );
                                                    if c.head_damage_mul > 1.0 {
                                                        stat_row(
                                                            ui,
                                                            "Tête",
                                                            &format!("×{:.1}", c.head_damage_mul),
                                                        );
                                                    }
                                                });
                                        }
                                        None => {
                                            ui.label(
                                                egui::RichText::new(
                                                    "stats indisponibles (genome non chargé)",
                                                )
                                                .size(14.0)
                                                .color(C_TEXT_MUTED),
                                            );
                                        }
                                    }

                                    if let Some(e) = element {
                                        let (best, worst) = strong_weak(&elem_cfg, e);
                                        ui.add_space(6.0);
                                        ui.separator();
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Fort vs   {}   ×{:.1}",
                                                arch_fr(best.0),
                                                best.1
                                            ))
                                            .size(15.0)
                                            .color(C_HP_HIGH),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Faible vs {}   ×{:.1}",
                                                arch_fr(worst.0),
                                                worst.1
                                            ))
                                            .size(15.0)
                                            .color(C_TEXT_MUTED),
                                        );
                                    }

                                    // ── Navigation par BOUTONS ◄ ► (clavier ← → toujours dispo) ──
                                    ui.add_space(12.0);
                                    ui.separator();
                                    ui.add_space(8.0);
                                    ui.vertical_centered(|ui| {
                                        ui.horizontal(|ui| {
                                            // Centrage manuel : la rangée prend toute la largeur →
                                            // on pousse de (largeur - contenu)/2 à gauche.
                                            let lead =
                                                ((ui.available_width() - 280.0) * 0.5).max(0.0);
                                            ui.add_space(lead);
                                            let prev = ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("‹")
                                                            .size(30.0)
                                                            .strong(),
                                                    )
                                                    .min_size(egui::vec2(64.0, 42.0)),
                                                )
                                                .on_hover_text("Arme précédente")
                                                .clicked();
                                            ui.add_space(10.0);
                                            ui.label(
                                                egui::RichText::new("Changer d'arme")
                                                    .size(15.0)
                                                    .color(C_TEXT_MUTED),
                                            );
                                            ui.add_space(10.0);
                                            let next = ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("›")
                                                            .size(30.0)
                                                            .strong(),
                                                    )
                                                    .min_size(egui::vec2(64.0, 42.0)),
                                                )
                                                .on_hover_text("Arme suivante")
                                                .clicked();
                                            if prev {
                                                choice.idx = (sel + n - 1) % n;
                                            }
                                            if next {
                                                choice.idx = (sel + 1) % n;
                                            }
                                        });
                                        // Déblocage CLIQUABLE si l'arme courante est verrouillée.
                                        if !owned {
                                            if let Some(u) = unlock {
                                                ui.add_space(8.0);
                                                let afford = meta.current >= u.cost;
                                                let mut unlock_clicked = false;
                                                ui.add_enabled_ui(afford, |ui| {
                                                    unlock_clicked = ui
                                                        .add(
                                                            egui::Button::new(
                                                                egui::RichText::new(format!(
                                                                    "Débloquer ({} Âmes)",
                                                                    u.cost
                                                                ))
                                                                .size(16.0)
                                                                .strong(),
                                                            )
                                                            .min_size(egui::vec2(220.0, 36.0)),
                                                        )
                                                        .clicked();
                                                });
                                                if unlock_clicked {
                                                    meta.current -= u.cost;
                                                    save.unlock_weapon(key);
                                                    save.souls_total = meta.current;
                                                    save.save();
                                                }
                                                if !afford {
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "{} Âmes manquantes",
                                                            u.cost.saturating_sub(meta.current)
                                                        ))
                                                        .size(12.0)
                                                        .color(C_TEXT_MUTED),
                                                    );
                                                }
                                            }
                                        }
                                    });
                                });
                            });
                    });
                });
        });
}

/// Rend la **carte d'arme du hub-menu** dans un `Ui` donné : aperçu 3D (image RTT
/// passée par l'appelant, ou placeholder si `None`) + nom/élément/stats/matchup +
/// sélecteur ‹ › + déblocage cliquable. Réutilise les helpers privés du module
/// (`persona`, `elem_color`, `strong_weak`, `stat_row`…). Mute `choice` (sélection)
/// et `save`/`meta` (déblocage) + sauve.
///
/// Note : rendu voisin de `draw_weapon_select` (Lobby) mais viewport différent
/// (image RTT ici vs viewport 3D transparent au Lobby) → panneau dédié assumé,
/// unifiable plus tard (dette tech mineure, story-menu-hub).
#[allow(clippy::too_many_arguments)]
pub fn draw_weapon_menu_panel(
    ui: &mut egui::Ui,
    choice: &mut StartingWeaponChoice,
    cards: &WeaponCards,
    elem_cfg: &ElementConfig,
    save: &mut MetaShopSave,
    cat: &MetaShopCatalogue,
    meta: &mut MetaSouls,
    weapon_image: Option<egui::TextureId>,
    image_size: f32,
) {
    let n = ARENA_V1_WEAPONS.len();
    let sel = choice.idx % n;
    let w = ARENA_V1_WEAPONS[sel];
    let key = vm_key(w);
    let owned = save.is_weapon_unlocked(key);
    let unlock = cat.weapon_unlock(key);
    let accent = if owned {
        crate::hud::speaker_color(weapon_to_speaker(w))
    } else {
        C_TEXT_MUTED
    };
    let (name, tagline) = persona(w);
    let card = cards.cards.get(&w);
    let element = elem_cfg.element_for(w);

    ui.set_min_width(400.0);
    ui.vertical_centered(|ui| {
        // ── Aperçu 3D (image RTT) OU placeholder ──
        match weapon_image {
            Some(tex) => {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    tex,
                    egui::vec2(image_size, image_size),
                )));
            }
            None => {
                let (r, _) = ui.allocate_exact_size(
                    egui::vec2(image_size, image_size),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(r, egui::CornerRadius::same(8), egui::Color32::from_black_alpha(120));
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "Aperçu 3D…",
                    egui::FontId::proportional(16.0),
                    C_TEXT_MUTED,
                );
            }
        }
        ui.add_space(10.0);

        // En-tête : nom + index parcouru + tagline.
        ui.horizontal(|ui| {
            ui.heading(display_text(name, 28.0, accent).strong());
            ui.label(
                egui::RichText::new(format!("‹ {}/{} ›", sel + 1, n))
                    .size(16.0)
                    .color(FORGE_TEAL),
            );
        });
        ui.label(
            egui::RichText::new(tagline)
                .size(14.0)
                .italics()
                .color(C_TEXT_MUTED),
        );

        // Statut verrou + niveau de maîtrise.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if owned {
                ui.label(
                    egui::RichText::new("DÉBLOQUÉE")
                        .size(15.0)
                        .strong()
                        .color(C_HP_HIGH),
                );
            } else {
                ui.label(
                    egui::RichText::new("VERROUILLÉE")
                        .size(15.0)
                        .strong()
                        .color(C_TEXT_MUTED),
                );
            }
            // Niveau EFFECTIF (borné au plafond) — cf draw_weapon_select.
            let lvl = cat.mastery.effective_level(save.weapon_level(key));
            let bonus = (cat.mastery.damage_mul(lvl) - 1.0) * 100.0;
            let cap = cat.mastery.max_level;
            ui.label(
                egui::RichText::new(format!("·  Niveau {lvl}/{cap}  (+{bonus:.0}% dégâts)"))
                    .size(15.0)
                    .strong()
                    .color(FORGE_OR),
            );
        });
        ui.add_space(8.0);

        if let Some(e) = element {
            ui.label(
                egui::RichText::new(format!("Élément : {} — {}", e.fr_name(), e.tag()))
                    .size(16.0)
                    .strong()
                    .color(elem_color(e, elem_cfg)),
            );
            ui.add_space(8.0);
        }

        match card {
            Some(c) => {
                let dps = if c.damage <= 0.0 {
                    "roquette AOE".to_string()
                } else {
                    format!("{:.0}", weapon_dps(c.damage, c.fire_rate, c.pellets))
                };
                let dmg = if c.damage <= 0.0 {
                    "—".to_string()
                } else {
                    format!("{:.0}", c.damage)
                };
                egui::Grid::new("forgia_ws_menu_grid")
                    .num_columns(2)
                    .spacing([18.0, 5.0])
                    .show(ui, |ui| {
                        stat_row(ui, "DMG / coup", &dmg);
                        stat_row(ui, "Cadence", &format!("{:.1} /s", c.fire_rate));
                        stat_row_strong(ui, "DPS", &dps, FORGE_OR);
                        stat_row(ui, "Chargeur", &c.mag_size.to_string());
                        stat_row(ui, "Recharge", &format!("{:.2} s", c.reload_time_secs));
                        stat_row(ui, "Portée", &format!("{:.0} m", c.range));
                        if c.head_damage_mul > 1.0 {
                            stat_row(ui, "Tête", &format!("×{:.1}", c.head_damage_mul));
                        }
                    });
            }
            None => {
                ui.label(
                    egui::RichText::new("stats indisponibles (genome non chargé)")
                        .size(14.0)
                        .color(C_TEXT_MUTED),
                );
            }
        }

        if let Some(e) = element {
            let (best, worst) = strong_weak(elem_cfg, e);
            ui.add_space(6.0);
            ui.separator();
            ui.label(
                egui::RichText::new(format!("Fort vs   {}   ×{:.1}", arch_fr(best.0), best.1))
                    .size(15.0)
                    .color(C_HP_HIGH),
            );
            ui.label(
                egui::RichText::new(format!("Faible vs {}   ×{:.1}", arch_fr(worst.0), worst.1))
                    .size(15.0)
                    .color(C_TEXT_MUTED),
            );
        }

        // Sélecteur d'arme ‹ › (mute `choice`).
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let prev = ui
                .add(egui::Button::new(egui::RichText::new("‹").size(28.0).strong()).min_size(egui::vec2(56.0, 40.0)))
                .on_hover_text("Arme précédente")
                .clicked();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Changer d'arme").size(15.0).color(C_TEXT_MUTED));
            ui.add_space(8.0);
            let next = ui
                .add(egui::Button::new(egui::RichText::new("›").size(28.0).strong()).min_size(egui::vec2(56.0, 40.0)))
                .on_hover_text("Arme suivante")
                .clicked();
            if prev {
                choice.idx = (sel + n - 1) % n;
            }
            if next {
                choice.idx = (sel + 1) % n;
            }
        });

        // Déblocage cliquable si l'arme est verrouillée (coûte des Âmes).
        if !owned {
            if let Some(u) = unlock {
                ui.add_space(8.0);
                let afford = meta.current >= u.cost;
                let mut unlock_clicked = false;
                ui.add_enabled_ui(afford, |ui| {
                    unlock_clicked = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("Débloquer ({} Âmes)", u.cost))
                                    .size(16.0)
                                    .strong(),
                            )
                            .min_size(egui::vec2(220.0, 36.0)),
                        )
                        .clicked();
                });
                if unlock_clicked {
                    meta.current -= u.cost;
                    save.unlock_weapon(key);
                    save.souls_total = meta.current;
                    save.save();
                }
                if !afford {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} Âmes manquantes",
                            u.cost.saturating_sub(meta.current)
                        ))
                        .size(12.0)
                        .color(C_TEXT_MUTED),
                    );
                }
            }
        }
    });
}

fn stat_row(ui: &mut egui::Ui, label: &str, val: &str) {
    ui.label(egui::RichText::new(label).size(15.0).color(C_TEXT_MUTED));
    // Story-617 — couleur explicite : sans elle la valeur héritait du texte egui
    // par défaut (sombre) → invisible sur le panneau noir (seul DPS, coloré, sortait).
    ui.label(
        egui::RichText::new(val)
            .size(15.0)
            .strong()
            .color(C_TEXT_LIGHT),
    );
    ui.end_row();
}

fn stat_row_strong(ui: &mut egui::Ui, label: &str, val: &str, col: egui::Color32) {
    ui.label(egui::RichText::new(label).size(15.0).color(C_TEXT_MUTED));
    ui.label(egui::RichText::new(val).size(17.0).strong().color(col));
    ui.end_row();
}

// ─── Aperçu 3D de l'arme (parentée caméra, tourne) — story-614 ───────────────

/// Distance (m) de l'arme devant la caméra (réglable si trop loin/près).
const PREVIEW_DIST: f32 = 1.6;
/// Position cible (m, camera-local) du CENTRE de l'arme — le recentrage AABB place
/// le centre géométrique de chaque arme ici. Calé sur le viewport (haut de la carte
/// centrée) → l'arme est centrée verticalement dans son cadre, pour toutes les armes.
const PREVIEW_Y: f32 = 0.70;
/// Taille cible (plus grande dimension, m) après calibrage AABB — calibrée pour que
/// l'arme tienne ENTIÈREMENT dans le viewport (pas de coupe aux bords).
const PREVIEW_TARGET: f32 = 0.80;
/// Hauteur (px) du viewport transparent en haut de la carte où l'aperçu 3D apparaît
/// (exception layout cosmétique de no-hardcode). L'arme (layer 0) rend DERRIÈRE egui
/// → visible dans ce vide ; le panneau stats opaque est dessous.
const WEAPON_VIEWPORT_H: f32 = 250.0;
/// Hauteur (px) FIXE du panneau stats → carte de taille identique pour toutes les
/// armes (rendu design cohérent ; layout cosmétique, exception no-hardcode).
const STATS_PANEL_H: f32 = 360.0;
/// Vitesse de rotation (rad/s).
const PREVIEW_SPIN: f32 = 0.9;

/// Marqueur sur une arme 3D d'aperçu (parentée caméra). Porte l'arme représentée
/// → le toggle de visibilité montre uniquement la sélectionnée (story-618).
#[derive(Component)]
struct LobbyPreviewWeapon {
    weapon: WeaponType,
}

/// Scène GLB de l'aperçu = enfant du pivot `LobbyPreviewWeapon`, recentrée (-center)
/// + scalée par `sys_calibrate_preview` → l'arme tourne AUTOUR DE SON CENTRE.
#[derive(Component)]
struct LobbyPreviewScene;

/// Sphère de pré-chauffe d'un matériau d'élément : rendue 1× au Lobby (occluse par
/// l'arme) pour compiler son pipeline `unlit/blend` AVANT le 1er impact en combat
/// (sinon freeze au 1er hit élémentaire). Despawn à la sortie du Lobby (story-618).
#[derive(Component)]
struct LobbyPrewarmSphere;

/// Calibrage AABB en attente (taille GLB native inconnue) — miroir merchant/boss_portal.
#[derive(Component)]
struct NeedsPreviewCalibrate {
    target: f32,
}

/// Story-618 — les 4 aperçus sont spawnés UNE fois à l'entrée du Lobby (plus de
/// despawn/respawn par ‹ › = plus de ré-instanciation de scène/hitch) ; le ‹ › ne
/// fait que **toggle la visibilité** (cycle instantané).
#[derive(Resource, Default)]
struct PreviewState {
    spawned: bool,
}

/// Walk les descendants → `(min, max)` de l'AABB combinée (espace local du root,
/// approx : ignore les transforms intermédiaires, suffisant pour des GLB d'arme).
/// Sert au scale (extent max) ET au RECENTRAGE vertical dans le viewport.
fn preview_aabb_bounds(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(a) = q_aabb.get(e) {
            let c = Vec3::from(a.center);
            let h = Vec3::from(a.half_extents);
            min = min.min(c - h);
            max = max.max(c + h);
            found = true;
        }
        if let Ok(children) = q_children.get(e) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }
    found.then_some((min, max))
}

/// Spawn / swap l'arme 3D selon la sélection, parentée à la caméra 3D active.
fn sys_lobby_weapon_preview(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<PreviewState>,
    q_cam: Query<(Entity, &Camera), With<Camera3d>>,
    vfx: Option<Res<ElementVfxAssets>>,
) {
    if state.spawned {
        return;
    }
    // Caméra 3D active = parent. Pas encore prête → retry frame suivante.
    let Some((cam, _)) = q_cam.iter().find(|(_, c)| c.is_active) else {
        return;
    };
    // Spawn les 4 armes UNE fois (toutes cachées ; le toggle révèle la sélectionnée).
    for w in ARENA_V1_WEAPONS {
        let key = vm_key(w);
        let scene = asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(format!("models/weapons/forgia/{key}.glb")));
        // Pivot = point de rotation + position (PREVIEW_Y, devant la caméra). La scène
        // GLB est un enfant RECENTRÉ dessus → l'arme tourne autour de son centre (pas
        // d'orbite) et est centrée X/Y quelle que soit l'origine du GLB.
        let pivot = commands
            .spawn((
                Name::new(format!("LobbyPreview_{key}")),
                LobbyPreviewWeapon { weapon: w },
                Transform::from_xyz(0.0, PREVIEW_Y, -PREVIEW_DIST),
                Visibility::Hidden,
                ChildOf(cam),
            ))
            .id();
        commands.spawn((
            Name::new(format!("LobbyPreviewScene_{key}")),
            LobbyPreviewScene,
            NeedsPreviewCalibrate {
                target: PREVIEW_TARGET,
            },
            SceneRoot(scene),
            // Échelle initiale minuscule → pas de flash géant avant calibrage AABB.
            Transform::from_scale(Vec3::splat(0.001)),
            Visibility::Inherited,
            ChildOf(pivot),
        ));
    }
    // Pré-chauffe des 4 matériaux d'élément (unlit/blend) : rendus 1× (sphères
    // minuscules, occluses par l'arme) → leur pipeline compile au Lobby, pas au
    // 1er impact élémentaire en combat. Despawn à la sortie du Lobby.
    if let Some(vfx) = vfx {
        for mat in &vfx.mats {
            commands.spawn((
                Name::new("LobbyPrewarmElem"),
                LobbyPrewarmSphere,
                Mesh3d(vfx.sphere.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(0.0, PREVIEW_Y, -PREVIEW_DIST).with_scale(Vec3::splat(0.03)),
                ChildOf(cam),
            ));
        }
    }
    state.spawned = true;
    info!("[weapon-select] aperçu 3D : 4 armes + pré-chauffe éléments (spawn-once, story-618)");
}

/// Story-618 — toggle la visibilité : seule l'arme sélectionnée est visible. Cycle
/// instantané (aucun spawn/despawn). La 1ère apparition de chaque arme paie sa
/// compile de pipeline une fois, puis c'est caché.
fn sys_toggle_preview_visibility(
    choice: Res<StartingWeaponChoice>,
    hub: Res<crate::hub::HubTab>,
    mut q: Query<(&LobbyPreviewWeapon, &mut Visibility)>,
) {
    let sel = ARENA_V1_WEAPONS[choice.idx % ARENA_V1_WEAPONS.len()];
    // L'aperçu d'arme ne s'affiche que sur l'onglet ARMES (vitrine de la carte) ;
    // sur Forge on montre les bras, ailleurs rien (home-hub P2.1).
    let on_armes = *hub == crate::hub::HubTab::Armes;
    for (pw, mut vis) in &mut q {
        let want = if on_armes && pw.weapon == sel {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Calibre l'échelle une fois l'AABB chargée (miroir `sys_calibrate_merchant`).
fn sys_calibrate_preview(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsPreviewCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
) {
    for (e, needs) in &q_needs {
        let Some((min, max)) = preview_aabb_bounds(e, &q_aabb, &q_children) else {
            continue; // scène pas encore chargée → retry
        };
        let max_dim = (max - min).max_element();
        if max_dim > 0.0 && max_dim.is_finite() {
            if let Ok(mut tf) = q_tf.get_mut(e) {
                let scale = needs.target / max_dim;
                tf.scale = Vec3::splat(scale);
                // Recentrage COMPLET (X/Y/Z) : place le centre géométrique de l'arme sur
                // l'origine du pivot → arme centrée ET rotation autour de son centre
                // (pas d'orbite), quelle que soit l'origine du GLB.
                let center = (min + max) * 0.5;
                tf.translation = -center * scale;
            }
        }
        commands.entity(e).remove::<NeedsPreviewCalibrate>();
    }
}

/// Fait tourner l'arme 3D sur son axe (turntable).
fn sys_spin_lobby_preview(time: Res<Time>, mut q: Query<&mut Transform, With<LobbyPreviewWeapon>>) {
    let d = PREVIEW_SPIN * time.delta_secs();
    for mut t in &mut q {
        t.rotate_local_y(d);
    }
}

/// OnExit(Lobby) — despawn l'aperçu + reset l'état (respawn propre au retour Lobby).
fn sys_clear_lobby_preview(
    mut commands: Commands,
    mut state: ResMut<PreviewState>,
    q: Query<Entity, Or<(With<LobbyPreviewWeapon>, With<LobbyPrewarmSphere>)>>,
) {
    for e in &q {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
    state.spawned = false;
}

// ─── Crosshair off au Lobby (story-617) ─────────────────────────────────────

/// OnEnter(Lobby) — masque le crosshair FPS : il n'a aucun sens sur l'écran de
/// sélection et traversait le modèle 3D. No-op si la Resource est absente.
fn sys_hide_crosshair_at_lobby(hidden: Option<ResMut<forgia_crosshair::CrosshairHidden>>) {
    if let Some(mut h) = hidden {
        h.0 = true;
    }
}

/// OnExit(Lobby) — réaffiche le crosshair (la run démarre = combat).
fn sys_show_crosshair_off_lobby(hidden: Option<ResMut<forgia_crosshair::CrosshairHidden>>) {
    if let Some(mut h) = hidden {
        h.0 = false;
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct WeaponSelectPlugin;

impl Plugin for WeaponSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StartingWeaponChoice>();
        app.init_resource::<WeaponCards>();
        app.add_systems(Startup, sys_load_weapon_cards);
        app.add_systems(
            Update,
            sys_hot_reload_weapon_cards
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            (sys_weapon_select_input, sys_weapon_unlock_input)
                .in_set(GameSet::UI)
                .run_if(in_state(RunState::Lobby)),
        );
        // Story-613 — en run, les armes verrouillées ne sont pas jouables (le switch
        // Digit2-4 est annulé avant le tir). Tourne en Movement (avant Combat).
        // `sys_sync_mastery_current` suit IMMÉDIATEMENT le garde de loadout : celui-ci
        // peut ramener `current` sur une arme débloquée, et la maîtrise doit refléter
        // l'arme retenue, pas celle qui vient d'être annulée. Les deux tournent en
        // Movement, donc AVANT le recompute des mods (Effects) — pas de lag d'une frame.
        app.add_systems(
            Update,
            (sys_enforce_unlocked_loadout, sys_sync_mastery_current)
                .chain()
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Hub à onglets (P2) : le wizard d'arme ne s'affiche que sur l'onglet ARMES.
        // L'aperçu 3D reste toujours visible (hero du hub), seul le panneau egui est gaté.
        app.add_systems(
            EguiPrimaryContextPass,
            draw_weapon_select.run_if(crate::hub::on_armes_tab),
        );
        // Aperçu 3D de l'arme (parentée caméra, tourne) — story-614.
        app.init_resource::<PreviewState>();
        app.add_systems(
            Update,
            (
                sys_lobby_weapon_preview,
                sys_toggle_preview_visibility,
                sys_calibrate_preview,
                sys_spin_lobby_preview,
            )
                .in_set(GameSet::Effects)
                .run_if(in_state(RunState::Lobby)),
        );
        app.add_systems(OnEnter(RunState::Lobby), sys_hide_crosshair_at_lobby);
        app.add_systems(
            OnExit(RunState::Lobby),
            (
                sys_apply_weapon_choice,
                sys_clear_lobby_preview,
                sys_show_crosshair_off_lobby,
            ),
        );
        // P3 — niveau de maîtrise par arme : +1 à chaque fin de run (Defeat/Victory).
        app.add_systems(OnEnter(RunState::Defeat), sys_level_up_equipped_weapon);
        app.add_systems(OnEnter(RunState::Victory), sys_level_up_equipped_weapon);
    }
}

// ─── Tests (logique pure) ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dps_matches_viewmodel_values() {
        // Pépin 28×6 = 168 ; Bourrasque 11×11 = 121.
        assert!((weapon_dps(28.0, 6.0, 1) - 168.0).abs() < 1e-3);
        assert!((weapon_dps(11.0, 11.0, 1) - 121.0).abs() < 1e-3);
    }

    #[test]
    fn dps_pellets_multiply_and_clamp() {
        assert!((weapon_dps(10.0, 1.0, 9) - 90.0).abs() < 1e-3);
        // pellets 0 clampé à 1 (pas de DPS nul artificiel).
        assert!((weapon_dps(10.0, 1.0, 0) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn vm_keys_distinct_and_mapped() {
        assert_eq!(vm_key(WeaponType::ModernAR), "pepin");
        assert_eq!(vm_key(WeaponType::AssaultRifle), "bourrasque");
        assert_eq!(vm_key(WeaponType::Shotgun), "madame_lenoir");
        assert_eq!(vm_key(WeaponType::RocketLauncher), "boucherie");
    }

    /// Régression du désync de maîtrise mesuré en jeu le 2026-08-04.
    ///
    /// Sauvegarde réelle observée : `pepin` niveau 13 (plafonné à 6 → ×1,20),
    /// `boucherie` niveau 2 (→ ×1,04). Le bonus appliqué était celui de l'AUTRE
    /// arme, dans les deux sens. Ce test fixe les deux valeurs : si elles cessaient
    /// de différer, le désync redeviendrait invisible.
    #[test]
    fn deux_armes_de_niveaux_differents_ne_donnent_pas_la_meme_maitrise() {
        let cat = MetaShopCatalogue::default();
        let pepin = cat.mastery.damage_mul(13); // plafonné à max_level
        let boucherie = cat.mastery.damage_mul(2);
        assert!(
            (pepin - boucherie).abs() > 1e-6,
            "sans écart mesurable, un bonus figé sur la mauvaise arme passerait inaperçu"
        );
        // Le plafond tient : un niveau 13 ne vaut pas plus qu'un niveau max_level.
        assert!((pepin - cat.mastery.damage_mul(cat.mastery.max_level)).abs() < 1e-6);
        assert!(pepin > boucherie, "plus de runs = plus de bonus");
    }

    #[test]
    fn parse_reads_real_viewmodel_shape() {
        // Sous-ensemble réel (champs ADS/juice ignorés par serde).
        let toml = r#"
[weapons.pepin]
target_size = 0.9
damage = 28.0
fire_rate = 6.0
range = 80.0
pellets = 1
mag_size = 12
reload_time_secs = 1.2
head_damage_mul = 2.0
ads_fov_deg = 30.0
"#;
        let cards = parse_cards(toml);
        let p = cards.get(&WeaponType::ModernAR).unwrap();
        assert_eq!(p.damage, 28.0);
        assert_eq!(p.mag_size, 12);
        assert!((p.head_damage_mul - 2.0).abs() < 1e-3);
    }

    #[test]
    fn parse_garbage_falls_back_to_mirror() {
        let cards = parse_cards("ceci n'est pas du toml [[[");
        assert_eq!(cards.len(), 4);
        assert!(cards.contains_key(&WeaponType::ModernAR));
    }

    #[test]
    fn mirror_default_has_four_distinct() {
        let m = mirror_default();
        assert_eq!(m.len(), 4);
        // Boucherie = roquette (damage 0 → étiquette AOE côté UI).
        assert_eq!(m.get(&WeaponType::RocketLauncher).unwrap().damage, 0.0);
    }

    #[test]
    fn strong_weak_armor_pierce_best_vs_tank() {
        let cfg = ElementConfig::default();
        let (best, worst) = strong_weak(&cfg, Element::ArmorPierce);
        assert!(matches!(best.0, EnemyArchetype::Tank));
        assert!(best.1 >= 2.0);
        assert!(best.1 > worst.1);
    }

    #[test]
    fn default_choice_is_pepin() {
        let c = StartingWeaponChoice::default();
        assert_eq!(ARENA_V1_WEAPONS[c.idx], WeaponType::ModernAR);
    }
}
