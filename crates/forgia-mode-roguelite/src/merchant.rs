//! merchant.rs — Story-610. Commerçant d'arène : sink in-run (Or + Âmes).
//!
//! Une entité FIXE dans l'arène (ne bouge jamais) où le joueur, à proximité,
//! achète au clavier (touches 1..N). Pendant a run (InRun/Boss) seulement —
//! distinct de L'Enclume (`meta_shop.rs`, Lobby, upgrades permanents).
//!
//! ## Économie (décision user 2026-06-20 — story-610)
//! - **Consommables → Or** (`Gold` = `forgia_rpg_data::loot_tables::Souls`),
//!   monnaie in-run sans sink jusqu'ici. Munitions, soin.
//! - **Premium → Âmes** (`MetaSouls`). « Second souffle » (revive token).
//!   Dépense conservée à la mort (flush meta_shop réconcilie `souls_total`).
//!
//! ## Data-driven
//! Catalogue = miroir EXACT de `assets/genomes/roguelite/roguelite_merchant.toml`
//! (pattern `meta_shop.rs`). Fallback `Default` si parse KO / liste vide.
//!
//! ## Patterns réutilisés (vérifiés sans édition cross-crate)
//! - Refill mag+réserve : miroir `stations::sys_use_ammo_stations`.
//! - Heal : miroir `stations::sys_use_health_stations` (`forgia_damage::Health`).
//! - Panneau + input clavier : miroir `meta_shop::draw_meta_shop_lobby` / `_input`.
//!
//! ## Revive & piège 2-Health (tracé sur le code)
//! `ReviveTokens` consommé par `run::obs_roguelite_player_death` AVANT l'émission
//! de `Defeat`. Le joueur ne porte QUE `forgia_damage::Health` (spawn
//! forgia-player/lib.rs:294 = `DamageHealth::new(100)` ; `forgia_combat::Health`
//! est ennemis-only — vérifié sur tout le crate : poi.rs/elements.rs/shockwave.rs).
//! Ce Health EST la source du `DeathEvent` → le revive le restaure, point.
//! (Note : `reference_two_health_types_combat_vs_damage` dit « player porte
//! combat::Health » ; le code le contredit — le player est damage-only.)

use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody};
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::*;
use forgia_player::Player;
use forgia_rpg_data::loot_tables::Souls as Gold;
use serde::Deserialize;
use std::fs;

use crate::run::{MetaSouls, RogueliteRunMarker, RunState};

/// État d'ouverture de la fenêtre du forgeron (dialogue). Toggle par E près du
/// marchand (`forge_shop.rs`). Ouvert → curseur libre + tir/look bloqués + fenêtre.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ForgeShopOpen(pub bool);

/// Marqueur du PNJ gobelin (pour l'anim procédurale — il n'a pas de rig).
#[derive(Component)]
pub struct MerchantVendor;

/// Demande d'achat d'un item du catalogue (index). Émise par le bouton de la fenêtre
/// OU la touche 1-N ; consommée par `sys_apply_purchase`.
#[derive(Message, Debug, Clone, Copy)]
pub struct PurchaseRequest {
    pub index: usize,
}

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_merchant.toml";
const SENSOR_PATH: &str = "forgia2_merchant.json";

/// Position FIXE du commerçant dans l'arène (« ne bouge jamais »). Devant-gauche
/// du spawn, hors du dais central (portail boss). Layout — tunable.
const MERCHANT_POS: Vec3 = Vec3::new(-10.0, 0.0, 12.0);
/// Rayon d'interaction (m) — panneau + achats actifs en deçà. Cf
/// `stations::STATION_TRIGGER_RADIUS` (précédent walk-over généreux).
const MERCHANT_RADIUS: f32 = 4.0;

// ─── Visuel GLB (étale KayKit medieval + PNJ Gobli, assets déjà en local) ────
/// Étale = bâtiment marché du pack KayKit Medieval Hexagon (CC0).
const STALL_GLB: &str = "models/kaykit/medieval_hexagon/buildings/red/building_market_red.gltf";
/// PNJ vendeur = gobelin marchand (asset perso existant).
const GOBLI_GLB: &str = "models/characters/Gobli.glb";
/// Taille cible (plus grande dimension) de l'étale, m — calibrée AABB (taille
/// GLB native inconnue, comme le portail). Tunable (3.4 → 6.0 : « trop petit »).
const STALL_TARGET_SIZE: f32 = 6.0;
/// Taille cible du PNJ (sa hauteur ≈ sa plus grande dim), m. Tunable.
const GOBLI_TARGET_SIZE: f32 = 1.6;
/// Offset LOCAL du PNJ vs l'étale. +Z = DEVANT (côté joueur), -Z = dans la boutique.
/// Réglé à l'œil : -0.6 (dans le shop) → +3.0 (devant le comptoir, face client).
const GOBLI_LOCAL_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 3.0);
/// Décalage de yaw de l'ensemble (l'étale + le PNJ regardent le spawn). L'avant
/// natif des GLB est supposé +Z → ajuster (`PI`, `±FRAC_PI_2`) après 1er rendu.
const STALL_YAW_OFFSET: f32 = std::f32::consts::PI;
/// Demi-emprise (m) du collider bloquant de l'étale (le joueur ne traverse pas).
/// Suit l'agrandissement de l'étale (STALL_TARGET_SIZE 6.0).
const STALL_COLLIDER_HALF: Vec3 = Vec3::new(2.2, 1.6, 1.6);

// ─── Monnaie & effets ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    /// Or in-run (`Gold`), perdu à la mort.
    Or,
    /// Âmes méta (`MetaSouls`), persistantes.
    Ames,
}

impl Currency {
    fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "or" | "gold" => Some(Currency::Or),
            "ames" | "âmes" | "souls" => Some(Currency::Ames),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Currency::Or => "Or",
            Currency::Ames => "Âmes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MerchantEffect {
    /// Recharge mag + réserve de l'arme en main.
    AmmoCurrent,
    /// Recharge mag + réserve de TOUTES les armes.
    AmmoAll,
    /// +N PV (clamp max).
    Heal(f32),
    /// +1 jeton « Second souffle » (survit à la prochaine mort).
    Revive,
}

impl MerchantEffect {
    fn from_key(key: &str, amount: f32) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "ammo_current" | "ammo" => Some(MerchantEffect::AmmoCurrent),
            "ammo_all" | "ammo_full" => Some(MerchantEffect::AmmoAll),
            "heal" | "soin" => Some(MerchantEffect::Heal(amount)),
            "revive" | "second_souffle" => Some(MerchantEffect::Revive),
            _ => None,
        }
    }
}

// ─── Catalogue (data-driven, miroir const) ───────────────────────────────────

#[derive(Clone, Debug)]
pub struct MerchantItem {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub currency: Currency,
    pub cost: u32,
    pub effect: MerchantEffect,
}

#[derive(Resource, Clone, Debug)]
pub struct MerchantCatalogue {
    pub items: Vec<MerchantItem>,
}

impl Default for MerchantCatalogue {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_merchant.toml.
        Self {
            items: vec![
                MerchantItem {
                    id: "ammo".into(),
                    name: "Munitions".into(),
                    desc: "Recharge la réserve de l'arme en main".into(),
                    currency: Currency::Or,
                    cost: 30,
                    effect: MerchantEffect::AmmoCurrent,
                },
                MerchantItem {
                    id: "ammo_all".into(),
                    name: "Réassort complet".into(),
                    desc: "Recharge TOUTES les armes".into(),
                    currency: Currency::Or,
                    cost: 70,
                    effect: MerchantEffect::AmmoAll,
                },
                MerchantItem {
                    id: "heal".into(),
                    name: "Soin".into(),
                    desc: "+40 PV".into(),
                    currency: Currency::Or,
                    cost: 40,
                    effect: MerchantEffect::Heal(40.0),
                },
                MerchantItem {
                    id: "revive".into(),
                    name: "Second souffle".into(),
                    desc: "Survis à ta prochaine mort (1×)".into(),
                    currency: Currency::Ames,
                    cost: 15,
                    effect: MerchantEffect::Revive,
                },
            ],
        }
    }
}

#[derive(Deserialize)]
struct ItemToml {
    id: String,
    name: String,
    desc: String,
    currency: String,
    cost: u32,
    effect: String,
    #[serde(default)]
    amount: f32,
}

#[derive(Deserialize)]
struct CatalogueToml {
    #[serde(default)]
    items: Vec<ItemToml>,
}

impl MerchantCatalogue {
    /// Pur — testable. Fallback `Default` si parse KO ou liste vide.
    pub fn parse_toml(content: &str) -> Self {
        let Ok(parsed) = toml::from_str::<CatalogueToml>(content) else {
            return Self::default();
        };
        let items: Vec<MerchantItem> = parsed
            .items
            .into_iter()
            .filter_map(|it| {
                let currency = Currency::from_key(&it.currency)?;
                let effect = MerchantEffect::from_key(&it.effect, it.amount)?;
                Some(MerchantItem {
                    id: it.id,
                    name: it.name,
                    desc: it.desc,
                    currency,
                    cost: it.cost,
                    effect,
                })
            })
            .collect();
        if items.is_empty() {
            Self::default()
        } else {
            Self { items }
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

// ─── État runtime (sensor + proximité + tokens) ──────────────────────────────

/// Marqueur de l'entité commerçant (1 seule, fixe).
#[derive(Component, Debug)]
pub struct Merchant;

/// Jetons « Second souffle » détenus. Consommés au décès (intercept Defeat).
/// Reset à 0 en rentrant au Lobby (insurance per-run, non reportée).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ReviveTokens(pub u32);

/// État agrégé pour le sensor + flag de proximité (lu par input/draw).
#[derive(Resource, Default, Debug, Clone)]
pub struct MerchantStats {
    pub near_player: bool,
    pub purchases_total: u32,
    pub revives_granted: u32,
    pub last_purchase: String,
}

// ─── Systems ─────────────────────────────────────────────────────────────────

/// `OnEnter(GameMode::Roguelite)` — spawn l'étale (GLB KayKit market) + le PNJ
/// Gobli à `MERCHANT_POS`, face au spawn. Le parent porte le marqueur `Merchant`
/// et un collider bloquant (indépendant du chargement async du GLB). Les visuels
/// GLB sont calibrés (scale AABB) puis posés au sol (`sys_calibrate/ground_merchant`).
pub fn sys_spawn_merchant(mut commands: Commands, asset_server: Res<AssetServer>) {
    let stall = asset_server.load(GltfAssetLabel::Scene(0).from_asset(STALL_GLB));
    let gobli = asset_server.load(GltfAssetLabel::Scene(0).from_asset(GOBLI_GLB));
    // L'étale + le PNJ regardent le spawn (origine). Avant natif GLB supposé +Z
    // → STALL_YAW_OFFSET (PI) ; ajuster après 1er rendu si de profil/dos.
    let yaw = MERCHANT_POS.x.atan2(MERCHANT_POS.z) + STALL_YAW_OFFSET;
    let parent = commands
        .spawn((
            Name::new("RogueliteMerchant"),
            Merchant,
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            RigidBody::Fixed,
            Transform::from_translation(MERCHANT_POS).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    // Collider bloquant (cuboïde, dimensionné const) — fiable même si le GLB n'est
    // pas encore chargé (cf boss_portal : collider primitif au spawn). L'interaction
    // marche donc même si le visuel GLB échoue à charger.
    commands.spawn((
        ChildOf(parent),
        Name::new("MerchantCollider"),
        Transform::from_xyz(0.0, STALL_COLLIDER_HALF.y, 0.0),
        Collider::cuboid(
            STALL_COLLIDER_HALF.x,
            STALL_COLLIDER_HALF.y,
            STALL_COLLIDER_HALF.z,
        ),
    ));
    // Étale GLB (scale calibré + posée au sol).
    commands.spawn((
        ChildOf(parent),
        Name::new("MerchantStallVisual"),
        SceneRoot(stall),
        Transform::IDENTITY,
        NeedsMerchantCalibrate {
            target: STALL_TARGET_SIZE,
            base_world_y: MERCHANT_POS.y,
        },
    ));
    // PNJ Gobli derrière le comptoir — regarde le joueur via le yaw du parent.
    // `MerchantVendor` → anim procédurale (rotation seule : le GLB n'a pas de rig).
    commands.spawn((
        ChildOf(parent),
        Name::new("MerchantVendorGobli"),
        MerchantVendor,
        SceneRoot(gobli),
        Transform::from_translation(GOBLI_LOCAL_OFFSET),
        NeedsMerchantCalibrate {
            target: GOBLI_TARGET_SIZE,
            base_world_y: MERCHANT_POS.y,
        },
    ));
    info!(
        "[merchant] étale + Gobli spawned @ {:?} (yaw {:.0}°)",
        MERCHANT_POS,
        yaw.to_degrees()
    );
}

// ─── Calibration + grounding GLB (adapté de boss_portal, story-603) ──────────

/// Posé sur un SceneRoot (étale/PNJ) : `sys_calibrate_merchant` mesure l'AABB une
/// fois la scène chargée et applique `scale = target / max_dim` (GLB de taille
/// native inconnue), puis passe le relais à `NeedsMerchantGround`.
#[derive(Component)]
struct NeedsMerchantCalibrate {
    target: f32,
    base_world_y: f32,
}

/// Après calibration (scale propagé), décale le SceneRoot en Y pour que la base
/// réelle de la géométrie repose sur `base_world_y` (corrige pivot GLB non au pied).
#[derive(Component)]
struct NeedsMerchantGround {
    base_world_y: f32,
}

/// Walk récursif des Children pour le 1er `Aabb` ; `max(half_extents)*2`.
/// (Dupliqué de boss_portal — extraction crate partagée si 3e consommateur.)
fn merchant_aabb_max_dim(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<f32> {
    if let Ok(a) = q_aabb.get(root) {
        return Some(a.half_extents.max_element() * 2.0);
    }
    let children = q_children.get(root).ok()?;
    let mut max = 0.0_f32;
    let mut found = false;
    for child in children.iter() {
        if let Some(d) = merchant_aabb_max_dim(child, q_aabb, q_children) {
            max = max.max(d);
            found = true;
        }
    }
    found.then_some(max)
}

/// Walk récursif : min Y monde sur tous les `Aabb` du sous-arbre (8 coins
/// transformés par leur `GlobalTransform`, robuste aux rotations).
fn merchant_min_world_y(
    e: Entity,
    q_children: &Query<&Children>,
    q_gt_aabb: &Query<(&GlobalTransform, &Aabb)>,
    acc: &mut f32,
    found: &mut bool,
) {
    if let Ok((gt, aabb)) = q_gt_aabb.get(e) {
        let c = Vec3::from(aabb.center);
        let he = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = c + Vec3::new(sx * he.x, sy * he.y, sz * he.z);
                    *acc = acc.min(gt.transform_point(corner).y);
                }
            }
        }
        *found = true;
    }
    if let Ok(children) = q_children.get(e) {
        for child in children.iter() {
            merchant_min_world_y(child, q_children, q_gt_aabb, acc, found);
        }
    }
}

/// Scale chaque SceneRoot taggé à `target` une fois l'AABB chargée, puis arme le
/// grounding. Miroir de `boss_portal::sys_calibrate_portal`.
fn sys_calibrate_merchant(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsMerchantCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
) {
    for (e, needs) in &q_needs {
        let Some(max_dim) = merchant_aabb_max_dim(e, &q_aabb, &q_children) else {
            continue; // scène pas encore chargée → retry next frame
        };
        if max_dim <= 0.0 || !max_dim.is_finite() {
            commands.entity(e).remove::<NeedsMerchantCalibrate>();
            continue;
        }
        let scale = needs.target / max_dim;
        if let Ok(mut tf) = q_tf.get_mut(e) {
            tf.scale = Vec3::splat(scale);
        }
        commands
            .entity(e)
            .remove::<NeedsMerchantCalibrate>()
            .insert(NeedsMerchantGround {
                base_world_y: needs.base_world_y,
            });
    }
}

/// Pose la base réelle du GLB sur `base_world_y` (sol). Miroir de
/// `boss_portal::sys_ground_portal`.
fn sys_ground_merchant(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsMerchantGround)>,
    q_children: Query<&Children>,
    q_gt_aabb: Query<(&GlobalTransform, &Aabb)>,
    mut q_tf: Query<&mut Transform>,
) {
    for (root, ground) in &q_needs {
        let mut min_y = f32::MAX;
        let mut found = false;
        merchant_min_world_y(root, &q_children, &q_gt_aabb, &mut min_y, &mut found);
        if !found {
            continue; // GlobalTransform/Aabb pas encore propagés → retry
        }
        let delta = ground.base_world_y - min_y;
        if let Ok(mut tf) = q_tf.get_mut(root) {
            tf.translation.y += delta;
        }
        commands.entity(root).remove::<NeedsMerchantGround>();
    }
}

/// `GameSet::Input` — met à jour `near_player` (distance² joueur ↔ commerçant).
pub fn sys_merchant_proximity(
    q_player: Query<&Transform, With<Player>>,
    q_merchant: Query<&Transform, With<Merchant>>,
    mut stats: ResMut<MerchantStats>,
) {
    let (Ok(player), Ok(merchant)) = (q_player.single(), q_merchant.single()) else {
        stats.near_player = false;
        return;
    };
    let d_sq = (merchant.translation - player.translation).length_squared();
    stats.near_player = d_sq <= MERCHANT_RADIUS * MERCHANT_RADIUS;
}

/// `GameSet::UI` — touches 1..N = achat (bonus clavier), UNIQUEMENT quand la fenêtre
/// du forgeron est ouverte. Émet `PurchaseRequest` (comme les boutons souris).
pub fn sys_merchant_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    shop: Res<ForgeShopOpen>,
    cat: Res<MerchantCatalogue>,
    mut ev: MessageWriter<PurchaseRequest>,
) {
    if !shop.0 {
        return;
    }
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let Some(i) = DIGITS.iter().position(|k| keys.just_pressed(*k)) else {
        return;
    };
    if i < cat.items.len() {
        ev.write(PurchaseRequest { index: i });
    }
}

/// Applique les `PurchaseRequest` (bouton souris ou clavier) : débit monnaie + effet.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_purchase(
    mut reqs: MessageReader<PurchaseRequest>,
    cat: Res<MerchantCatalogue>,
    mut stats: ResMut<MerchantStats>,
    mut gold: Option<ResMut<Gold>>,
    mut meta: ResMut<MetaSouls>,
    mut equipped: Option<ResMut<EquippedWeapons>>,
    mut revive: ResMut<ReviveTokens>,
    q_player: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    for req in reqs.read() {
    let Some(item) = cat.items.get(req.index) else {
        continue;
    };

    // Solde dans la bonne monnaie ?
    let balance = match item.currency {
        Currency::Or => gold.as_deref().map(|g| g.current).unwrap_or(0),
        Currency::Ames => meta.current,
    };
    if balance < item.cost {
        info!(
            "[merchant] pas assez de {} pour {} ({}/{})",
            item.currency.label(),
            item.name,
            balance,
            item.cost
        );
        continue;
    }

    // Débit.
    match item.currency {
        Currency::Or => {
            if let Some(g) = gold.as_deref_mut() {
                g.current -= item.cost;
            }
        }
        Currency::Ames => {
            meta.current -= item.cost;
        }
    }

    // Effet.
    match item.effect {
        MerchantEffect::AmmoCurrent => {
            if let Some(eq) = equipped.as_deref_mut() {
                let current = eq.current;
                if let Some(slot) = eq.slots.get_mut(&current) {
                    slot.current_mag = slot.config.mag_size;
                    slot.reserve = slot.config.reserve_max;
                } else {
                    warn!(
                        "[merchant] Munitions : aucun slot pour l'arme courante {:?} — débit sans effet",
                        current
                    );
                }
            }
        }
        MerchantEffect::AmmoAll => {
            if let Some(eq) = equipped.as_deref_mut() {
                for slot in eq.slots.values_mut() {
                    slot.current_mag = slot.config.mag_size;
                    slot.reserve = slot.config.reserve_max;
                }
            }
        }
        MerchantEffect::Heal(amount) => {
            if let Ok(player_e) = q_player.single() {
                // Miroir stations heal — défère pour éviter conflit de Query.
                commands.queue(move |world: &mut World| {
                    if let Some(mut hp) = world.get_mut::<forgia_damage::Health>(player_e) {
                        hp.current = (hp.current + amount).min(hp.max);
                    }
                });
            }
        }
        MerchantEffect::Revive => {
            revive.0 += 1;
            stats.revives_granted += 1;
        }
    }

    stats.purchases_total += 1;
    stats.last_purchase = item.id.clone();
    info!(
        "[merchant] acheté {} (-{} {})",
        item.name,
        item.cost,
        item.currency.label()
    );
    }
}

/// Reset des jetons revive en rentrant au Lobby (insurance per-run).
pub fn sys_reset_revive_tokens(mut revive: ResMut<ReviveTokens>) {
    revive.0 = 0;
}

/// Sensor `forgia2_merchant.json` 1Hz — soldes, proximité, achats, health check.
pub fn sys_write_merchant_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cat: Res<MerchantCatalogue>,
    stats: Res<MerchantStats>,
    revive: Res<ReviveTokens>,
    gold: Option<Res<Gold>>,
    meta: Res<MetaSouls>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let or_balance = gold.as_ref().map(|g| g.current).unwrap_or(0);
    // Échappe l'id (vient du TOML) pour ne pas corrompre le JSON sensor.
    let last = stats
        .last_purchase
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let (severity, next_step) = if cat.items.is_empty() {
        ("warn", "Catalogue commerçant vide — vérifier roguelite_merchant.toml")
    } else {
        ("ok", "")
    };
    let json = format!(
        r#"{{"id":"merchant","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"items_count":{},"near_player":{},"or_balance":{},"ames_balance":{},"revive_tokens":{},"purchases_total":{},"revives_granted":{},"last_purchase":"{}"}}"#,
        time.elapsed_secs(),
        cat.items.len(),
        stats.near_player,
        or_balance,
        meta.current,
        revive.0,
        stats.purchases_total,
        stats.revives_granted,
        last,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[merchant] sensor write failed: {e}");
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct MerchantPlugin;

impl Plugin for MerchantPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MerchantCatalogue::load_or_default());
        app.init_resource::<MerchantStats>();
        app.init_resource::<ReviveTokens>();
        app.init_resource::<ForgeShopOpen>();
        app.add_message::<PurchaseRequest>();
        app.add_systems(OnEnter(GameMode::Roguelite), sys_spawn_merchant);
        app.add_systems(OnEnter(RunState::Lobby), sys_reset_revive_tokens);
        // Calibration scale (AABB) + pose au sol des visuels GLB (étale + Gobli).
        app.add_systems(
            Update,
            (sys_calibrate_merchant, sys_ground_merchant)
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_merchant_proximity
                .in_set(GameSet::Input)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Achat = event-driven : clavier (bonus, si fenêtre ouverte) + boutons souris
        // (forge_shop.rs) → PurchaseRequest → sys_apply_purchase. La FENÊTRE elle-même
        // (dialogue E, curseur, colonnes, anim gobelin) vit dans `forge_shop.rs`.
        app.add_systems(
            Update,
            (sys_merchant_keyboard, sys_apply_purchase)
                .chain()
                .in_set(GameSet::UI)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_merchant_sensor.in_set(GameSet::Sensors),
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalogue_has_both_currencies() {
        let cat = MerchantCatalogue::default();
        assert!(cat.items.iter().any(|i| i.currency == Currency::Or));
        assert!(cat.items.iter().any(|i| i.currency == Currency::Ames));
    }

    #[test]
    fn revive_item_uses_ames() {
        let cat = MerchantCatalogue::default();
        let rev = cat.items.iter().find(|i| i.id == "revive").unwrap();
        assert_eq!(rev.currency, Currency::Ames);
        assert_eq!(rev.effect, MerchantEffect::Revive);
    }

    #[test]
    fn ammo_items_use_or() {
        let cat = MerchantCatalogue::default();
        for id in ["ammo", "ammo_all"] {
            let it = cat.items.iter().find(|i| i.id == id).unwrap();
            assert_eq!(it.currency, Currency::Or);
        }
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        let c = MerchantCatalogue::parse_toml("pas du toml [[[");
        assert_eq!(c.items.len(), MerchantCatalogue::default().items.len());
    }

    #[test]
    fn parse_roundtrip_matches_genome() {
        // Miroir du genome embarqué : 4 items, dont revive en âmes.
        let toml = r#"
[[items]]
id = "ammo"
name = "Munitions"
desc = "x"
currency = "or"
cost = 30
effect = "ammo_current"
amount = 0.0

[[items]]
id = "revive"
name = "Second souffle"
desc = "y"
currency = "ames"
cost = 15
effect = "revive"
amount = 0.0
"#;
        let c = MerchantCatalogue::parse_toml(toml);
        assert_eq!(c.items.len(), 2);
        assert_eq!(c.items[0].effect, MerchantEffect::AmmoCurrent);
        assert_eq!(c.items[1].currency, Currency::Ames);
    }

    #[test]
    fn heal_effect_carries_amount() {
        assert_eq!(
            MerchantEffect::from_key("heal", 40.0),
            Some(MerchantEffect::Heal(40.0))
        );
    }

    #[test]
    fn currency_keys_parse() {
        assert_eq!(Currency::from_key("or"), Some(Currency::Or));
        assert_eq!(Currency::from_key("ames"), Some(Currency::Ames));
        assert_eq!(Currency::from_key("xxx"), None);
    }
}
