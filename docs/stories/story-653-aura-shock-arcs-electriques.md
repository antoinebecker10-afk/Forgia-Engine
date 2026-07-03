# Story-653 — Aura électrique : l'ennemi choqué grésille (identité Pépin)

> **Statut** : IN_PROGRESS (validation visuelle user en attente)
> **Niveau BMAD** : Quick (4 fichiers)
> **Origine** : audit VFX 2026-07-02 (chaîne élémentaire : « StatusShock = AUCUNE aura — l'ennemi électrisé est invisible alors que la vuln ×1.1 est active ») + demande user « go Pépin » 2026-07-03.

## Découverte concept-first

La « marque électrique » runtime n'est PAS un composant `StatusShock` (ça n'existe pas) : c'est **`forgia_damage::Vulnerability`** — posé par elements.rs au hit Shock (`elements.rs:1215`), retiré à l'expiration (`:1389`). `ShockParams` = la config genome seulement. L'aura s'accroche donc sur `Added<Vulnerability>` / `RemovedComponents<Vulnerability>`.

## Design

- **Crépitement, pas flamme** : bursts fréquents (4×count / 70 ms) d'étincelles très brèves (60-160 ms) à la SURFACE du corps, stoppées net (drag 6) — ça grésille, ça ne monte pas.
- Couleur bleu-blanc HDR (palette shock du genome elements), texture spark 4 branches (partagée, zéro asset nouveau).
- **Lisibilité gameplay** : l'aura EST la fenêtre de vulnérabilité (+10 % dégâts) — « tire sur celui qui grésille ».
- Pattern burn/poison à l'identique : cap partagé (48), suivi par frame, scale par archétype, `DespawnOnExit`, multiplicateurs `roguelite_vfx.toml` (story-652).

## Fichiers

- `forgia-effects/weapon_vfx/status.rs` — `create_status_shock` ; `mod.rs` + `lib.rs` (handle + warmup 12e dummy)
- `forgia-mode-roguelite/status_vfx.rs` — `StatusVfxKind::Shock`, `ShockVfxAttached`, attach/detach ; `lib.rs` wiring

## Acceptance criteria

- [x] Aura spawn sur `Added<Vulnerability>`, despawn sur retrait (expiration/mort)
- [x] Distincte des flammes : brève, bleue, surface, sans montée
- [x] Warmup shader (pas de freeze au 1er choc) ; compteur `dot_pulses` partagé
- [ ] **Validation user** : un ennemi touché par Pépin grésille visiblement pendant la vuln
- [x] check + build verts (clippy : 3e instance du pattern `live` préexistant du fichier — cleanup groupé candidat)

## Suite

Visuel **Miasma** (le dernier statut muet) · silhouettes de réaction distinctes (nova/arcs/nuage) — audit §6.
