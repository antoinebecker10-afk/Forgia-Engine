//! # Ammo systems — sync genome + reload state machine
//!
//! Story-455 Phase A (2026-05-18). Glue entre :
//! - `ViewmodelGenome` (assets/genomes/viewmodel_arena.toml) — source de vérité config ammo
//! - `forgia_combat::EquippedWeapons.slots` (HashMap<WeaponType, AmmoSlot>) — runtime state
//! - `MessageWriter<AmmoChanged>` — bus event UI/sensor
//!
//! Systèmes wired par `ForgiaFpsPlugin` :
//! - `sync_ammo_slots_from_genome` — Asset Created/Modified → push config dans slots
//! - `reload_key_input` — touche R (KeybindRegistry-ready, fallback KeyCode::KeyR)
//! - `tick_ammo_reload` — décrémente timers, émet events sur transfer
//! - `cancel_reload_on_weapon_switch` — switch arme → cancel reload arme précédente

use bevy::prelude::*;
use forgia_combat::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponType, ARENA_V1_WEAPONS};
use forgia_genome_core::Genome;

use forgia_viewmodel::{
    lookup_genome_entry, weapon_genome_key, ViewmodelGenome, ViewmodelGenomeEntry,
    ViewmodelGenomeHandle,
};

// ─── SystemParam bundle pour fire_weapon_minimal ───────────────────────────

/// Bundle ResMut+Writer pour consommation ammo dans le fire system.
/// Évite de dépasser la limite Bevy 16 params en groupant equipped + writer.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AmmoCtx<'w> {
    pub equipped: ResMut<'w, EquippedWeapons>,
    pub events: MessageWriter<'w, AmmoChanged>,
}

impl AmmoCtx<'_> {
    /// Tente de consommer 1 shot et émet event. Retourne true si tir autorisé.
    /// Si mag vide et reserve > 0 : auto-start reload + return false.
    /// Si mag vide et reserve == 0 : "click empty" silencieux (TODO sfx) + return false.
    ///
    /// **BUG-455-01 fix** : si slot pas encore init par genome async (boot < ~200ms),
    /// retourne `false` au lieu de laisser tirer en infinite-mode invisible. Le 1er
    /// shot autorisé sera juste après le `sync_ammo_slots_from_genome` (déjà dans le
    /// chain, runs avant fire_weapon_minimal cette frame).
    pub fn try_fire(&mut self) -> bool {
        let weapon = self.equipped.current;
        let Some(slot) = self.equipped.slots.get_mut(&weapon) else {
            // Slot pas encore peuplé → refuser le tir. La fenêtre de blocage est limitée
            // au temps de chargement du genome ViewmodelGenome (~50-200ms boot first time,
            // 0 sur hot-reload). Évite le "shoot before sync" hole.
            return false;
        };
        if !slot.can_fire() {
            // Auto-reload si possible.
            if slot.try_start_reload() {
                let snapshot =
                    AmmoChanged::snapshot(weapon, slot, AmmoChangeKind::Reload { transferred: 0 });
                self.events.write(snapshot);
            }
            return false;
        }
        // Si en plein reload (mag) et user tire : cancel reload + tire.
        // ShellPerShell : cancel + tire (interruptible attendu, AAA pump feel).
        if slot.reload_state.is_reloading() {
            slot.cancel_reload();
        }
        slot.consume_shot();
        let snapshot = AmmoChanged::snapshot(weapon, slot, AmmoChangeKind::Fire { shots: 1 });
        self.events.write(snapshot);
        true
    }
}

// ─── System : sync genome → slots ──────────────────────────────────────────

/// Extrait l'`AmmoConfig` d'un `ViewmodelGenomeEntry`.
fn config_from_entry(e: &ViewmodelGenomeEntry) -> AmmoConfig {
    AmmoConfig {
        mag_size: e.mag_size,
        reserve_max: e.reserve_max,
        reload_time_secs: e.reload_time_secs,
        reload_kind: ReloadKind::from_genome_str(&e.reload_kind),
        infinite_ammo: e.infinite_ammo,
        low_ammo_threshold: e.low_ammo_threshold,
    }
}

/// Push les configs ammo du genome dans chaque slot d'EquippedWeapons.
/// Run quand l'asset ViewmodelGenome est Created OU Modified (hot-reload).
///
/// Stratégie défensive : si le handle n'est pas encore là (boot précoce), no-op silent.
/// Le system continuera de re-tenter chaque tick jusqu'au load complete.
pub fn sync_ammo_slots_from_genome(
    mut events: MessageReader<AssetEvent<Genome<ViewmodelGenome>>>,
    handle: Option<Res<ViewmodelGenomeHandle>>,
    assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut equipped: ResMut<EquippedWeapons>,
    mut ammo_events: MessageWriter<AmmoChanged>,
) {
    let Some(handle) = handle else { return };
    let mut should_sync = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => {
                if *id == handle.0.id() {
                    should_sync = true;
                }
            }
            _ => {}
        }
    }
    if !should_sync {
        return;
    }
    for weapon in ARENA_V1_WEAPONS.iter().copied() {
        let Some(entry) = lookup_genome_entry(&assets, &handle, weapon) else {
            warn!(
                "[ammo-sync] genome entry absent pour '{}' (clé '{}') — slot non sync",
                core::any::type_name::<WeaponType>(),
                weapon_genome_key(weapon),
            );
            continue;
        };
        let cfg = config_from_entry(entry);
        sync_ammo_slot_from_config(&mut equipped.slots, weapon, cfg, &mut ammo_events);
        info!(
            "[ammo-sync] {:?} → mag={} reserve_max={} reload={:.2}s kind={:?} infinite={}",
            weapon,
            cfg.mag_size,
            cfg.reserve_max,
            cfg.reload_time_secs,
            cfg.reload_kind,
            cfg.infinite_ammo,
        );
    }
}

// ─── System : reload key (R) ───────────────────────────────────────────────

/// Touche R démarre un reload manuel sur l'arme courante.
/// (KeybindRegistry pas encore wired V2 → fallback `KeyCode::KeyR` direct.
/// TODO story-456 : migrer vers `KeybindRegistry::action_just_pressed("reload")`.)
pub fn reload_key_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut equipped: ResMut<EquippedWeapons>,
    mut events: MessageWriter<AmmoChanged>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let weapon = equipped.current;
    let Some(slot) = equipped.slots.get_mut(&weapon) else {
        return;
    };
    if slot.try_start_reload() {
        let snap = AmmoChanged::snapshot(weapon, slot, AmmoChangeKind::Reload { transferred: 0 });
        events.write(snap);
    }
}

// ─── System : tick reload timers ──────────────────────────────────────────

/// Tick chaque slot en cours de reload. Émet `AmmoChanged::Reload` sur transfer.
/// Indépendant de l'arme courante : un slot non-équipé peut continuer son reload
/// (pas le cas en V2 — pas de pickup multi-clip — mais futur-compatible).
///
/// **BUG-455-03 fix** : `Local<Vec>` réutilisé + `.clear()/.extend()` au lieu de
/// `Vec::new()` chaque frame (alloc=0 hot path, Bevy Cheat Book par_iter convention).
pub fn tick_ammo_reload(
    time: Res<Time>,
    mut equipped: ResMut<EquippedWeapons>,
    mut events: MessageWriter<AmmoChanged>,
    mut keys_buf: Local<Vec<WeaponType>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Snapshot des clés dans un Local<Vec> réutilisé (évite double-borrow ET alloc/frame).
    keys_buf.clear();
    keys_buf.extend(equipped.slots.keys().copied());
    for w in keys_buf.iter().copied() {
        let Some(slot) = equipped.slots.get_mut(&w) else {
            continue;
        };
        if let Some(kind) = slot.tick_reload(dt) {
            let snap = AmmoChanged::snapshot(w, slot, kind);
            events.write(snap);
        }
    }
}

// ─── System : cancel reload on weapon switch ──────────────────────────────

/// Quand `EquippedWeapons.current` change, cancel le reload de l'arme précédente.
/// AAA pattern : weapon switch interrompt reload (Apex/CoD/Halo).
/// Émet `WeaponSwitch` event pour HUD refresh.
pub fn cancel_reload_on_weapon_switch(
    mut equipped: ResMut<EquippedWeapons>,
    mut events: MessageWriter<AmmoChanged>,
    mut last_current: Local<Option<WeaponType>>,
) {
    let current = equipped.current;
    let prev = *last_current;
    if prev == Some(current) {
        return;
    }
    // Cancel reload de l'ancienne arme.
    if let Some(prev_weapon) = prev {
        if let Some(slot) = equipped.slots.get_mut(&prev_weapon) {
            if slot.reload_state.is_reloading() {
                slot.cancel_reload();
                let snap = AmmoChanged::snapshot(prev_weapon, slot, AmmoChangeKind::WeaponSwitch);
                events.write(snap);
            }
        }
    }
    // Snapshot état nouvelle arme.
    if let Some(slot) = equipped.slots.get(&current) {
        let snap = AmmoChanged::snapshot(current, slot, AmmoChangeKind::WeaponSwitch);
        events.write(snap);
    }
    *last_current = Some(current);
}
