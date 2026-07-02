# Story-648 — Paliers de hitstop : hit < crit < kill < multikill

> **Statut** : IN_PROGRESS (validation runtime user en attente)
> **Niveau BMAD** : Quick+ (3 fichiers code + 1 genome + story)
> **Origine** : audit VFX 2026-07-02 §P0-3 — « le hitstop existe mais uniforme (0.05 s) ; le hitstop des KILLS doit être nettement plus long que celui des hits » (réfs SF2 ~166 ms, ULTRAKILL 0.2-0.5 s, Vlambeer 1-2 frames/hit).

## Design

- **Base par-arme inchangée** (`hit_stop_duration` genome arme) : hit normal = ×1.0 → zéro régression de feel (no-speculative-fix).
- **Multiplicateurs genome** (`roguelite_gamefeel.toml`, hot-reload 1Hz) : crit/weakspot ×1.5, kill ×2.5, multikill ×4.0 (≥2 kills dans le MÊME tir — détection synchrone, pas de fenêtre glissante → story future si besoin).
- Base 0.05 s → 50 / 75 / 125 / 200 ms.
- Kill compté à l'**edge** vivant→mort (un ennemi touché par plusieurs pellets = 1 kill).
- UI intacte : le tick hitstop consomme `Time<Real>`, la sim `Time<Virtual>` (existant).

## Fichiers

- `crates/forgia-juice-lib/src/hit_stop.rs` — `HitstopTiers` (parse+clamps), `tier_for_shot` (pur), `HitstopStats`, hot-reload, capteur `forgia2_gamefeel.json` ; +8 tests
- `crates/forgia-juice-lib/Cargo.toml` — dep `toml`
- `crates/forgia-fps/src/lib.rs` — agrégation par tir (`shot_kills` edge-detected, `shot_crit_or_head`) + palier au site de déclenchement ; resources via `FireTimingCtx` (limite 16 params respectée)
- `assets/genomes/roguelite/roguelite_gamefeel.toml` — 3 genes

## Acceptance criteria

- [x] Hit normal : durée strictement identique à avant (×1.0)
- [x] Paliers ordonnés hit < crit < kill < multikill (test `tiers_ordered_ascending`)
- [x] Kill edge-detected (pas de double comptage multi-pellet sur le même ennemi)
- [x] Genome hot-reload + capteur `forgia2_gamefeel.json` (counts par palier + last_tier/duration)
- [ ] **Validation runtime user** : un kill « croque » nettement plus qu'un hit
- [x] `cargo check` + clippy 0 warning introduit + tests verts

## Suite

P0-4 knockback par hit (audit §P0) · fenêtre multikill temporelle (si le feel synchrone ne suffit pas) · trauma additionnel au kill (CameraTrauma existant).
