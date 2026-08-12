# Story-651 — Chime weakspot + variation de pitch (l'oreille distingue hit/tête/kill)

> **Statut** : IN_PROGRESS (validation oreille user en attente)
> **Niveau BMAD** : Quick (1 fichier code + 1 asset + 2 TOML/docs)
> **Origine** : audit VFX 2026-07-02 §P0-5 — pattern Gunfire Reborn : « UN son unique réservé au weakspot, jamais utilisé ailleurs ; la cohérence croisée son/couleur crée le réflexe pavlovien de viser la tête ».

## Découverte concept-first

Le système audio combat **existait déjà** (story-559, `audio.rs` roguelite) : `impact.ogg` par hit, `kill.ogg` (meaty) par kill, tir par arme — le « thump kill » du plan était déjà livré. Les 2 manques réels :

1. **`is_headshot` ignoré** — une tête sonnait exactement comme un corps ;
2. **Zéro variation de pitch** — full-auto 16 tirs/s = métronome (fatigue auditive).

## Changements (`forgia-mode-roguelite/src/audio.rs`)

- **Chime weakspot** : `weakspot.ogg` (Kenney `impactMetal_light_000`, CC0 — « tink » métallique crisp), joué en **couche additive à CHAQUE headshot**, même au kill (double récompense Gunfire). **Pitch FIXE** : c'est la signature — reconnaissable entre mille, jamais variée.
- **Pitch ±5 %** sur les sons répétitifs (tirs + impacts) : xorshift `Local`, présentation only (hors sim/keystone). Kill et chime gardent leur pitch (identité).
- Genome `roguelite_audio.toml [weakspot]` (path + volume, hot-reload) ; capteur `forgia2_roguelite_audio.json` + champ `weakspots`.

## Hiérarchie audio résultante (école Destiny, audit §5)

tir (pitch varié) < impact hit (pitch varié) < **tink weakspot (fixe)** < thump kill < barks armes (quota).

## Acceptance criteria

- [x] Headshot → tink audible EN PLUS de l'impact/kill ; jamais sur un tir corps
- [x] Pitch tirs/impacts varie dans [0.95, 1.05] (test `pitch_variation_stays_in_bounds_and_varies`)
- [x] Chime et kill = pitch fixe (signatures)
- [x] Asset CC0 documenté (CREDITS.md) ; genome hot-reload ; capteur `weakspots`
- [x] **Le son part** — run du 2026-08-12, `forgia2_roguelite_audio.json` :
      `weakspots: 192` sur `impacts: 369` (52 %), au sein de `sfx_played: 2075`.
- [ ] **Validation oreille user** : le « tink » installe l'envie de viser la tête ; le full-auto ne fait plus métronome
      → *reste ouvert à dessein : la variation de pitch et l'effet « métronome »
      ne s'entendent pas dans un compteur.*
      ⚠️ **Anomalie voisine relevée au passage** (hors scope de cette story) :
      `roguelite_audio.kills: 2` alors que `knockback.kill_pushes: 51`. Le son de
      mort ne part quasiment jamais — cf story de défaut ouverte le 2026-08-12.
- [x] Tests + clippy + build verts

## Suite

Séquence de mort 4 temps (P1, gros morceau) · barks kill des armes qui parlent (chantier voix gibberish, rapport gunfire-like P0) — le moteur de barks existant devient la couche au-dessus du thump.
