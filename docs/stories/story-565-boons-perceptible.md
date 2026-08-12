# Story-565 — Rendre les boons perceptibles (sortir de l'Excel)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_boons.json`, fichier `boons_apply.rs`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (~5-7 fichiers)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 1/4 — fracture #2 de la critique GD 2026-05-29
> **Priorité GD** : **2** (débloque la rejouabilité par les builds)
> **Dépend de** : story-559 (particules/feedback) idéalement avant

---

## 1. Contexte

Critique GD : *"Les boons ne se SENTENT pas. C'est de l'Excel déguisé. Quand je
prends 'Métal chaud' (le feu !), il ne se passe RIEN à l'écran — +15% de dégâts
qu'aucun humain ne perçoit."*

### État vérifié

- 9 effets câblés (`boons_apply.rs:45-74`) : DamageMul, FireRateMul, HealOnKill,
  DamageReduction, Knockback, ChainTargets, FlatBonus — **tous numériques, 0 feedback visuel/comportemental.**
- Hadès/Isaac/StS : un boon **change ce que tu VOIS faire**. Ici un boon "feu",
  un boon "ricochet" et un boon "précision" produisent la même expérience : zéro.

---

## 2. Vision

**Aucun boon n'est un nombre invisible.** Chaque boon acheté a une **manifestation
perceptible** — visuelle, comportementale, ou sonore :

| Boon | Manifestation perceptible |
|---|---|
| Métal chaud (feu/dmg) | balles **enflammées** (tracer orange + petit burst feu au hit) |
| Ricochet | balles qui **rebondissent visiblement** |
| Chaîne (ChainTargets) | **éclair** qui saute entre ennemis |
| Soin au kill | **halo vert** + petit "+HP" pop |
| Knockback | ennemis **poussés** nettement + dust |
| Réduction dégâts | **bouclier scintillant** autour du joueur |
| Crit/précision | **flash + ✨** sur crit (extend hitmarker story-528) |

---

## 3. Acceptance Criteria

### AC1 — Mapping boon → effet perceptible data-driven ✅ **OBLIGATOIRE**
- Chaque boon déclare son `feedback_kind` dans `roguelite_boons.toml` (vfx/color/behavior tag)
- Pas de boon sans manifestation : si effet purement stat, lui donner au minimum une **couleur de tracer** ou un **icône d'état actif HUD**

### AC2 — Au moins 5 manifestations distinctes câblées ✅
- Balles enflammées (feu), éclair de chaîne, halo de soin, bouclier réduction, push knockback visible
- Réutiliser bevy_hanabi (story-559) + StandardMaterial emissive

### AC3 — État de build lisible sur le HUD ✅
- Petite barre d'icônes des boons actifs (le joueur voit sa build se construire)
- Au pick, micro-feedback "BOON ACQUIS" + l'effet se voit dès le tir suivant

### AC4 — Honnêteté des libellés ✅
- Corriger les boons qui mentent (ex `chaussons_du_lievre` : *"move_speed demande wire, reformulé via fire_rate"*) — soit câbler le vrai effet, soit renommer pour refléter ce qui se passe vraiment
- Aucun boon ne décrit un effet qu'il ne produit pas

### AC5 — Observability ✅ **OBLIGATOIRE**
- `forgia2_boons.json` : boons actifs + `feedback_emitted` par type (preuve que la manifestation tire)

---

## 4. Hot path check
- [ ] VFX par hit = effet hanabi poolé, pas spawn illimité
- [ ] Tracer coloré = matériau partagé par boon, pas `materials.add()` par tir
- [ ] HUD icônes = redraw seulement sur `Changed<ActiveBoons>`
- [ ] Systèmes gated `run_if(in_state(GameMode::Roguelite))`

---

## 5. Fichiers candidats (~5-7)

| Fichier | Rôle |
|---|---|
| `assets/genomes/roguelite/roguelite_boons.toml` | `feedback_kind` par boon + fix libellés |
| `crates/forgia-mode-roguelite/src/boons_apply.rs` | émettre le feedback au déclenchement de l'effet |
| `crates/forgia-mode-roguelite/src/boon_vfx.rs` (NEW) | manifestations hanabi/emissive |
| `crates/forgia-ui-lib/...` | barre d'icônes boons actifs HUD |
| `crates/forgia-observability/...` | sensor AC5 |

---

## 6. Test in-game (récap obligatoire)

1. **Action** : acheter un boon au Coffre, puis tirer.
2. **Redémarrage** : `cargo run`. `feedback_kind` → Shift+F12.
3. **Effet attendu** :
   - "Métal chaud" → dès le tir suivant, balles **orange enflammées**
   - "Soin au kill" → **halo vert** + "+HP" à chaque kill
   - HUD montre l'icône du boon acquis
4. **Sensor** : `forgia2_boons.json::feedback_emitted` incrémente par type
5. **Variantes si KO** :
   - Effet invisible → augmenter taille/durée du VFX
   - Tracer pas coloré → vérifier matériau partagé appliqué post-pick

---

## 7. Definition of Done
- [ ] AC1-AC5 livrés (≥5 manifestations distinctes)
- [ ] `cargo check` + clippy 0 warning
- [ ] Sub-agents verifier + qa-lead
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] Aucun boon ne ment sur son effet (AC4)
- [ ] Story DONE + ROADMAP mise à jour

## 8. Coupes assumées
- ❌ Boons qui rewire un verbe entier (dash→teleport, Hadès-tier) — V2
- ❌ Animation unique par boon — V1 = manifestation lisible suffit
