# Story-530 — 24 Boons catalogue + 3 Anti-boons (Mission 2 GDD)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_boons.json`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~5 jours
> **GDD ref** : [Mission 2.2](../design/gdd-roguelite-v1.md#22-catalogue-par-arme-12-boons) + [Mission 2.4](../design/gdd-roguelite-v1.md#24-anti-boons-3-marchés-forgeron-noir)
> **Prérequis** : story-529 (boons architecture + Coffre UI + tags + neutres)

## Pourquoi

24 boons V1 (12 par arme + 5 neutres déjà story-529 + 7 légendaires/synergies) = build variety pour 100+ builds possibles (Hadès doctrine). 3 anti-boons "marché Forgeron Noir" = curiosity hook + comic relief.

## Acceptance Criteria

### 12 boons par arme (3/arme)

- [ ] AC1 — Pépin : "Premier vrai tir", "Pépin s'enhardit", "Crénom de Pépin !" (jauge confiance impl)
- [ ] AC2 — Bourrasque : "Vent de chaos", "Souffle de Bourrasque", "Saperlipopette ça pète !" (chain damage radius)
- [ ] AC3 — Madame Lenoir : "Œil de Lenoir", "Mouchoir parfumé", "Une dame ne se précipite pas" (charge shot)
- [ ] AC4 — Boucherie : "BOUM extra-cuit", "Pirouette envoyée", "Boucherie joyeuse" (chain explosions)

### Synergies tagged (7 légendaires cachés)

- [ ] AC5 — Légendaire "Forge Ardente" : 3 boons `fire` → +flame VFX persistante tirs
- [ ] AC6 — Légendaire "Tempête de Ferraille" : 3 boons `ricochet` → projectiles rebondissent 3× au lieu 1×
- [ ] AC7 — Légendaire "Chaos Suprême" : 3 boons `chaos` → +20% mag size all weapons
- [ ] AC8 — Légendaire "Précision Mortelle" : 3 boons `precision` → 1er tir d'un mag = +200% damage
- [ ] AC9 — Légendaire "Onde de Choc" : 3 boons `knockback` → tous tirs knockback léger
- [ ] AC10 — Légendaire "Réaction en Chaîne" : 3 boons `chain` → kill propage 3 ennemis voisins
- [ ] AC11 — Légendaire mystère "Amitié des Armes" : combinaison 4 weapons-tags actives = bonus mystery

### 3 anti-boons Forgeron Noir

- [ ] AC12 — "Marché de Mauvais Goût" : post-process vision floue >15m + damage all +50% (toggle visual)
- [ ] AC13 — "Talon de Ferraille" : -1 cœur perma run + 3 boons coffre suivant
- [ ] AC14 — "Pacte Rigolo" : spawn × 3 ennemis 30s + loot éclats × 5

### Polish

- [ ] AC15 — Cinematic Forgeron Noir 5s mini-cutscene apparition aléatoire 1×/run après wave random, ridicule (Bowser-energy)
- [ ] AC16 — Sensor `forgia2_boons.json` étendu : légendaires unlocked + anti-boons accepted/refused stats

## Files
- `assets/genomes/roguelite_boons.toml` (extension)
- `crates/forgia-mode-roguelite/src/boons_effects.rs` NEW (implém mécanique par boon)
- `crates/forgia-mode-roguelite/src/anti_boons_cutscene.rs` NEW
- `crates/forgia-effects/src/forgeron_noir_cutscene.rs` NEW

## Cross-refs
- GDD V1 Mission 2.2 + 2.4
- story-531/532/533/534 (movesets armes — boons appliquent sur leurs verbes)
