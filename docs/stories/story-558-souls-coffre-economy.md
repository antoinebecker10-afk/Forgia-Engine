# Story-558 — Souls → Coffre du Forgeron Economy + Break 15s

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_coffre.json`, fichier `boons.rs`, symbole `BoonsCatalogue`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (≥10 fichiers, story requise, checklist post-impl)
> **Owner** : Claude Opus 4.7
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Audit source** : [docs/audit/roguelite-engagement-audit-2026-05-29.md](../audit/roguelite-engagement-audit-2026-05-29.md)

---

## 1. Contexte

Audit roguelite engagement (2026-05-29) a identifié que **Souls = compteur
vanity** : kill enemies → drop souls → walk-over collect → HUD compteur →
**rien**. Aucun consommateur. Le système économique core du genre roguelite
est manquant.

Story-529 Phase 1 (commit `6674a0e`) + Phase 2 (commit `4c46507`) ont shippé
une foundation boons (data + sensor + 5 neutral + 3 legendary) mais **sans
connexion à Souls** et **sans trigger runtime**.

User feedback explicite (2026-05-29) :
> "Faut l'améliorer et augmenter le temps entre les vagues à 15 sec
> le temps d'améliorer son stuff et préparer des munitions etc."

Best practices industry convergent (Hadès Charon, RoR2 chest, StS card
removal, Vampire Survivors level-up choice) : **break entre waves = window
shopping forcé + 3 choix forcés + preview before pick**.

---

## 2. Vision

Pendant le break 15s entre waves, **le Maître Forgeron apparaît** au
centre de l'arène (ou via overlay UI) et propose **3 boons** au joueur.
Chaque boon a un **coût en Souls**. Le joueur peut :
- **Acheter 1 boon** (consomme Souls, applique effet permanent dans la run)
- **Skip** (garde ses Souls pour la prochaine offre)
- **Reroll** (coût fixe ~30 souls, génère 3 nouveaux boons)

Bible cartoon : pas de marchand cynique. **Le Maître Forgeron est mentor,
encourageant**. Les boons sont nommés joliment ("Éclat d'âme nourrissant",
"Tornade de Braise"), pas en jargon stats.

---

## 3. Acceptance Criteria

### AC1 — Break entre waves passe de 3s à 15s ✅ **OBLIGATOIRE**

- `BREAK_SECS = 15.0` dans [waves.rs:32](../../crates/forgia-mode-roguelite/src/waves.rs#L32)
- HUD wave counter affiche `"NEXT IN 14.9s"` puis countdown
- Sensor `forgia2_roguelite_state.json::break_secs_left` reflète

### AC2 — Coffre du Forgeron apparaît OnEnter break ✅

- Système `sys_open_coffre_on_break` détecte transition `in_break: false → true`
- Spawn UI overlay (egui) : "Le Maître Forgeron t'offre 3 choix"
- 3 boons rolled depuis `BoonsCatalogue` (story-529 infra)
- Pondération : ~70% Common, ~25% Uncommon, ~5% Legendary (calibré genome)
- Boons offerts ne se dupliquent pas dans la même offre
- Boons déjà acquis dans la run **excluded du pool**

### AC3 — Boon a un coût en Souls ✅

- Nouveau champ `souls_cost: u32` dans `Boon` struct + genome TOML
- Defaults proposés :
  - Common = **20 souls** (~4 Tank kills)
  - Uncommon = **40 souls** (~8 Tank kills)
  - Legendary = **80 souls** (~16 Tank kills + Boss 40)
- Boon **désactivé/grisé** si `Souls.current < cost`
- Tooltip "Pas assez de souls" sur hover si insuffisant

### AC4 — Selection : decrement Souls + apply effect ✅

- Click sur un boon viable → `souls.current -= cost`
- Insert `BoonInstance` Resource (ou Component sur Player)
- `sys_apply_boon_effect` consume les boons actifs et applique stats:
  - `damage_mul` → `BotShootConfig` player weapon
  - `damage_reduction` → Player `incoming_damage_mul`
  - `heal_on_kill` → on DeathEvent enemy → `player.hp += value`
  - `fire_rate_mul` → cooldown player weapon
  - `headshot_mul` → multiplier HitZone::Head
  - `chain_targets`, `knockback`, `crit_chance` → respectifs systems
- Coffre **se ferme automatiquement** après pick (1 boon par break)

### AC5 — Skip et Reroll options ✅

- Bouton **"Passer"** ferme le Coffre sans dépense (garde Souls)
- Bouton **"Reforger l'offre"** : coût 30 souls, génère 3 nouveaux boons
- Disabled si `Souls.current < 30`
- Reroll **n'a pas de limite** par break (best practice Hadès reroll Diamond)

### AC6 — Pas de pool trash ✅

- Aucun boon "weak/situational". Tous les 8 boons actuels (+ futurs)
  doivent être pickable même sans synergie active.
- Test : pour chaque boon, justifier en commentaire TOML pourquoi il
  reste utile en standalone.

### AC7 — Preview Perfect Information ✅

- Sur hover boon → tooltip détaillé :
  - Nom + description courte (≤80 chars)
  - Effet exact en chiffres ("+15% damage")
  - Tags synergie iconisés
  - Coût Souls en gros
  - **Pas de RNG caché** (chain_targets affiche "3 cibles", pas "quelques")

### AC8 — Carry-over Souls sur Defeat ✅ (anti "mort = rien")

- Au lieu de reset `Souls = 0` sur restart, garde **25% rounded** des Souls.
- Defeat overlay affiche : "Tu gardes 12 souls de ta forge précédente."
- Best practice anti-pattern documenté (Game Developer "Hadès narrative
  rewards"). Cible enfants doit être encouragée, pas punie.

### AC9 — Sensor `forgia2_coffre.json` 1Hz ✅

- Schema :
  - `offered_boons: [{id, rarity, cost}]` actuel
  - `last_picked_id` + `last_picked_secs`
  - `souls_spent_total`
  - `reroll_count_run`
  - `coffre_open: bool`
- Severity warn si Coffre ouvert > 20s (joueur AFK / indécis)

### AC10 — Player HP restauré à 100% à chaque break ✅

- Au moment de l'ouverture du Coffre (`in_break: false → true`), `Player.Health.current = Player.Health.max`.
- HUD heart icons s'animent (refill visuel) — pattern Hadès "Charon's Boon".
- Justification bible cartoon : encourage le risk-taking, pas de "save HP for next wave". Chaque wave est un fresh combat.
- Heart drops mid-wave restent utiles (overheal interdit, mais permet de tenir la wave courante).
- Cohérent avec break 15s = window prep + heal = sanctuary moment.
- Sensor `forgia2_rpg_health.json::current` doit afficher max au tick d'entrée break.

### AC11 — Bible cartoon UI ✅

- Pas de panel sombre "MERCHANT". Plutôt fond bois clair, doré,
  illustrations chaudes.
- Texte "Le Maître Forgeron" pas "Shop" / "Vendor"
- Sound cue chaud sur ouverture (bell/forge clang)
- Couleur rareté **Diablo standard kid-friendly** : gris Common / vert
  Uncommon / violet Legendary (skip bleu — confond avec UI standard).

---

## 4. Phases d'implémentation

### Phase 1 — Break 15s + sensor de validation (Quick)
- [waves.rs:32](../../crates/forgia-mode-roguelite/src/waves.rs#L32) `BREAK_SECS = 15.0`
- Test compile + observer `forgia2_roguelite_state.json::break_secs_left = 15.0`
- ✅ User peut valider tout de suite

### Phase 2 — Souls cost in TOML + struct (Standard)
- Ajouter `souls_cost: u32` dans `Boon` struct ([forgia-rpg-data](../../crates/forgia-rpg-data/src/boons.rs) — vérifier path)
- Etendre [roguelite_boons.toml](../../assets/genomes/roguelite/roguelite_boons.toml) avec `souls_cost` par boon
- Parser le champ dans `parse_toml`
- Tests purs : parse + clamp

### Phase 3 — Coffre UI (Standard)
- Nouveau module `coffre.rs` dans `forgia-mode-roguelite`
- Resource `CoffreOffer { boons: Vec<BoonId>, picked: Option<BoonId> }`
- System `sys_open_coffre_on_break` : rolls 3 boons sur transition in_break false→true
- UI egui overlay : panel + 3 cards + Skip + Reroll buttons
- Hot-stop : Coffre input blocks player mouse_look (cf
  [[reference_input_blockers_anti_cycle_pattern]])

### Phase 4 — Apply boon effects (Standard)
- Resource `ActiveBoons: Vec<BoonInstance>` (additif, tag-tracked)
- Systems d'application par effect kind :
  - `sys_apply_damage_mul` (mute BotShootConfig player)
  - `sys_apply_heal_on_kill` (observe DeathEvent enemy)
  - `sys_apply_damage_reduction` (mute Player damage receive)
  - `sys_apply_fire_rate` (mute weapon cooldown)
  - `sys_apply_headshot_mul` (mute HitZone::Head multiplier)
  - `sys_apply_crit_chance` (player weapon crit roll)
  - `sys_apply_chain_targets` (forgia-fps hitscan chain mod)
  - `sys_apply_knockback` (DeathEvent impulse to corpse)
- Tests purs effet par effet

### Phase 5 — Carry-over Souls Defeat (Quick)
- Modifier reset Souls sur Defeat→Lobby : keep 25% rounded
- HUD Defeat overlay : message "Tu gardes X souls"
- Test sensor

### Phase 6 — Sensor + Telemetry (Quick)
- `forgia2_coffre.json` 1Hz writer (clone pattern [toon_config.rs](../../crates/forgia-mode-roguelite/src/toon_config.rs))
- Champs §AC9
- Severity warn AFK

### Phase 7 — Polish bible cartoon (Quick)
- UI panel woody/golden (bevy_egui style)
- Voiceline maître forgeron (stub TODO si BarkEvent encore disabled)
- Couleurs rareté Diablo kid-friendly

---

## 5. Files touchés (estimation)

| Fichier | Action | Phase |
|---|---|---|
| `crates/forgia-mode-roguelite/src/waves.rs` | `BREAK_SECS = 15.0` | 1 |
| `crates/forgia-mode-roguelite/src/coffre.rs` | NEW (~400 LOC) | 3-4 |
| `crates/forgia-mode-roguelite/src/lib.rs` | wire CoffrePlugin | 3 |
| `crates/forgia-mode-roguelite/src/hud.rs` | Defeat overlay carry-over msg | 5 |
| `crates/forgia-mode-roguelite/src/sensor.rs` | extend (optional) | 6 |
| `crates/forgia-rpg-data/src/boons.rs` (ou autre) | `souls_cost` field + parser | 2 |
| `assets/genomes/roguelite/roguelite_boons.toml` | +souls_cost par boon | 2 |
| `crates/forgia-rpg-data/src/loot_tables.rs` | carry-over Souls logic | 5 |
| Possibles : `crates/forgia-fps/`, `crates/forgia-combat/`, `crates/forgia-player/` | apply boon effects hooks | 4 |

Estimation : **8-14 fichiers** → BMAD **Standard** confirmé.

---

## 6. Invariants à protéger (Stability Locks)

- [ ] **L1 GameAssets** : aucun nouveau handle préload (Coffre UI peut
      utiliser textures procédurales / egui fonts existantes).
- [ ] **L7 SystemSets** : nouveaux systems dans `GameSet::Effects` ou `UI`.
- [ ] **LOCK-INV-1 Inventory** : aucun touch — boons sont Resource séparée.
- [ ] **Concept-First** : data-driven uniquement (TOML genome,
      pas hardcode), conforme `.claude/rules/no-hardcode.md`.
- [ ] **Multi-terminal** : ce travail concerne `forgia-mode-roguelite`
      crate + `forgia-rpg-data`. Coordonner si autre terminal touche.

---

## 7. Sensors à observer (in-game test recap)

### Phase 1 validation
- **Action** : entre Roguelite, kill wave 1 (8 enemies)
- **Effet** : HUD affiche "NEXT IN 14.X" puis countdown 15s → wave 2 spawn
- **Sensor** : `forgia2_roguelite_state.json::break_secs_left ~14.5` au début
- **KO** : si reste 3s → `BREAK_SECS` pas pris en compte (vérifier compile)

### Phase 4 validation
- **Action** : Coffre ouvert → click un boon viable (Souls > cost)
- **Effet visuel** : panel se ferme, Souls counter décrémenté, stat
  applied visible (ex : Métal chaud → damage tooltip viewmodel +15%)
- **Sensor** : `forgia2_coffre.json::last_picked_id` non-null +
  `souls_spent_total` += cost
- **Hot-reload** : édite `souls_cost` dans TOML → re-ouvre Coffre au prochain break → cost mis à jour

---

## 8. Anti-patterns à BANNIR (audit §8)

- ❌ Pool boons avec trash : tester chaque combinaison standalone viable
- ❌ Texte mur de stats : tooltip ≤80 chars en français bible-friendly
- ❌ Punition cosmétique Defeat : carry-over Souls + voiceline encourageante
- ❌ Méta-grind : un boon = un choix, pas un farm
- ❌ Tutorial popup : Coffre intuitif via UI hover (Perfect Info StS)
- ❌ Texte non-FR ou jargon : "souls" OK (bible), pas "DPS" / "RNG" / "boon"

---

## 9. Checklist post-implementation

À cocher avant déclaration DONE (cf `.bmad/checklists/post-implementation.md`) :

- [ ] `cargo check -p forgia-game` ✅ 0 erreur
- [ ] `cargo clippy -p forgia-mode-roguelite --no-deps` ✅ 0 warning
- [ ] `cargo clippy -p forgia-rpg-data --no-deps` ✅ 0 warning
- [ ] `cargo test -p forgia-mode-roguelite --lib` ✅ tous tests pass
- [ ] `cargo test -p forgia-rpg-data --lib` ✅ tous tests pass
- [ ] Sub-agent `verifier` ✅ Stability Locks intacts
- [ ] Sub-agent `qa-lead` ✅ BUG REPORT vide ou justifié
- [ ] Sensor `forgia2_coffre.json` écrit + valide JSON
- [ ] Hot-reload TOML souls_cost confirmé runtime
- [ ] Story `docs/stories/_index.md` statut → DONE
- [ ] `docs/ROADMAP_CURRENT.md` item coché
- [ ] In-game test recap fourni à l'user (§7)
- [ ] Memory candidate notée si pattern reproductible identifié

---

## 10. Cross-refs

- Audit : [docs/audit/roguelite-engagement-audit-2026-05-29.md](../audit/roguelite-engagement-audit-2026-05-29.md)
- Bible : [[reference_bible_forgia_roguelite_v1]]
- Industry gaps : [[reference_industry_3_gaps_forgia_roguelite]]
- Story-529 boons foundation (commits `6674a0e` + `4c46507`)
- Concept-First : `.claude/rules/concept-first.md` §6 (combat, inventory)
- Observability : `.claude/rules/observability-required.md`
- Input gating : [[reference_input_blockers_anti_cycle_pattern]]

---

## 11. Notes implementation

### Pondération roll boons

Roll formula recommandée (Hadès-aligned, kid-friendly bias) :
```
roll = xoshiro256(seed ^ break_n)
common_threshold = 70   // 0-69 = Common
uncommon_threshold = 95 // 70-94 = Uncommon
                        // 95-99 = Legendary
```

**3 premiers breaks d'un run** : forcer **au moins 1 Uncommon** dans
l'offre (calque Vampire Survivors hardcoded high-yield premiers chests).
Améliore onboarding cible enfants.

### Boon souls_cost suggestions (Phase 2 défauts)

| Boon | Rarity | Cost | Justification |
|---|---|---|---|
| Éclat d'âme nourrissant | Common | 20 | Heal 5/kill = strong basique |
| Métal chaud | Common | 20 | +15% damage = pillar build |
| Bénédiction Enclume | Common | 25 | Tank build viable seul |
| Souffle du Maître | Common | 25 | +20% fire rate = DPS scaling |
| Petit Champignon | Common | 20 | +10% crit = synergie pure |
| Tornade de Braise | Legendary | 80 | +75% damage = run-defining |
| Œil de Madame Lenoir | Legendary | 70 | +50% headshot dmg = skill cap |
| Ouragan de Bourrasque | Legendary | 70 | knockback 25 = defensive ult |
| Chaîne des Âmes | Legendary | 80 | chain 3 = AoE clear |

### UI panel egui (sketch)

```
╔══════════════════════════════════════════╗
║       LE MAÎTRE FORGERON T'OFFRE         ║
╠══════════════════════════════════════════╣
║  ┌────────┐  ┌────────┐  ┌────────┐     ║
║  │ Éclat  │  │ Métal  │  │Tornade │     ║
║  │ d'âme  │  │ chaud  │  │de Braise│    ║
║  │  5HP   │  │ +15%   │  │ +75%   │     ║
║  │ /kill  │  │ damage │  │ damage │     ║
║  │  20 ◇  │  │  20 ◇  │  │  80 ◇  │     ║
║  └────────┘  └────────┘  └────────┘     ║
║                                          ║
║   Tes souls : 47 ◇                       ║
║                                          ║
║  [ Passer ]      [ Reforger (30 ◇) ]    ║
╚══════════════════════════════════════════╝
```

(◇ = icône soul, panel bois clair fond doré conforme bible cartoon)

---

*Story créée 2026-05-29 — audit-driven priorité Tier 1.*
