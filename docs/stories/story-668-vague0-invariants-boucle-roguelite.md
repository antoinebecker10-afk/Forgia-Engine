# story-668 — Vague 0 : remettre les invariants de la boucle roguelite debout

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_boons.json`, fichier `boons.rs`, symbole `sys_start_run`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (implémentation livrée, validation runtime en attente)
**Niveau BMAD** : Standard (5 fichiers, 1 crate + 1 genome)
**Date** : 2026-07-31
**Related** : rapport [`docs/design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md`](../design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md), story-591 (méta-shop), story-597 (FTUE 1re mort), story-613 (déblocage d'armes), story-616 (paliers d'atouts), story-645 (stats de run)

---

## Demande

> « ok go » — sur la **Vague 0** du rapport de boucles roguelite : les quatre défauts
> classés P0, ceux qui cassent un invariant du genre plutôt qu'un réglage.

---

## Contexte — d'où viennent ces quatre points

L'audit du 2026-07-31 (5 cartographies du code + 5 vérifications adverses + 3 recherches
sourcées) a classé 22 ruptures de la boucle roguelite. Quatre sont **P0** : elles ne
rendent pas le jeu « moins bon », elles **cassent une propriété que le genre tient pour
acquise**, ou font perdre au joueur quelque chose qu'il a gagné.

Le reste (choix de porte inerte, composition figée, difficulté 100 % statistique,
absence de méta-méta…) est de la conception, pas du correctif — hors scope ici.

---

## Les 4 correctifs

### 1. 🔴 Une run repart de zéro (invariant du genre)

**Le défaut.** `ActiveBoons::reset_run()` (`forgia-rpg-data/src/boons.rs:242`) n'était
appelé **nulle part en production** — son unique appelant du workspace était son propre
test (`boons.rs:728`). `sys_start_run` remettait à zéro l'Or, la vague, les PV, le chrono
et le `CombatRng` — jamais les atouts.

**La conséquence.** À 6 coffres par run, un joueur qui meurt et relance garde tout : la 3ᵉ
run démarrait avec ~15 atouts, leurs compteurs de tags et les légendaires déjà
déverrouillés. La construction du build cessait d'être un enjeu de run pour devenir un
compteur de session. C'est la permadeath du roguelite, cassée par un appel manquant.

**Le correctif.** `crates/forgia-mode-roguelite/src/run.rs`, dans `sys_start_run`, juste
après le reset des PV du joueur.

**Pourquoi `commands.queue` et pas un `ResMut` de plus** : `sys_start_run` a **déjà
16 SystemParams**, le plafond dur de Bevy — un 17ᵉ ne compile pas (cf `scalability.md`
« SystemParam bundle quand > 12 params »). `commands.queue` est de surcroît l'idiome
déjà employé deux lignes plus haut pour remettre les PV au maximum.
`boons_apply::sys_recompute_boon_mods` tourne sur change-detection de `ActiveBoons` : il
recompose `PlayerCombatMods` seul après le flush des commandes.

### 2. 🔴 La maîtrise d'arme a un plafond, et il vit en couche definition

**Le défaut.** `+4 %` de dégâts par run **terminée — défaite comprise** — sans aucun
clamp, aucun `max_level`, et un multiplicateur non borné. À la 25ᵉ run avec la même arme :
**+96 % de dégâts permanents**, ce qui annule le scaling ennemi (+35 % PV/salle) et rend
la courbe de difficulté intenable. Le `const WEAPON_MASTERY_DMG_PER_LEVEL` portait
d'ailleurs son propre aveu : *« Balance — à externaliser en genome »*.

**Le correctif.**

- `assets/genomes/roguelite/roguelite_meta_shop.toml` : nouvelle section `[mastery]`
  (`max_level = 6`, `damage_per_level = 0.04` → **+20 % au plafond**, exactement la valeur
  que le commentaire d'origine visait).
- `meta_shop.rs` : `MasteryConfig` + `damage_mul(level)` **pur et clampé**, miroir Rust
  `Default`, parsing avec fallback par champ (`max_level = 0` → miroir Rust, un plafond
  nul est absurde).
- `MetaShopSave::level_up_weapon(key, max_level)` clampe et **retourne le niveau atteint**.
- `weapon_select.rs` : le `const` disparaît. Les **trois** sites (calcul runtime + deux
  affichages d'UI) lisent désormais la même source ; l'UI affiche `Niveau {lvl}/{cap}`.
  Un site qui diverge de la valeur runtime redevient impossible.
- `sys_level_up_equipped_weapon` sort tôt quand le niveau est inchangé → **plus d'écriture
  disque** une fois au plafond.

> **Effet sur les saves existantes** : une arme déjà au-dessus de 6 conserve son niveau
> stocké (rien n'est réécrit tant qu'aucune run ne se termine avec elle), mais son bonus
> **effectif** est ramené à +20 % par le clamp de `damage_mul`. C'est le but : la valeur
> non bornée était le défaut.

### 3. 🔴 Le FTUE n'enseigne plus le chemin qui ne marche pas

**Le défaut.** À la première mort, la flèche « ↑ dépense tes Âmes ici, puis repars » était
rendue **juste après le bouton REJOUER** (`hud.rs:484-499`) — donc affichée sous lui, donc
le désignant. Or **REJOUER** repart au Lobby, qui **auto-lance la run** : il n'y a aucune
occasion de dépenser quoi que ce soit sur ce chemin. L'Enclume est derrière **RETOUR AU
MENU**, le bouton du dessous. Le tout premier enseignement de méta-boucle du jeu pointait
le mauvais bouton.

**Le correctif.** Deux lignes, chacune sous le bouton qu'elle décrit :

- sous REJOUER → « ↑ repart tout de suite, sans rien acheter » (ton neutre, `FORGE_CREME`)
- sous RETOUR AU MENU → « ↑ L'Enclume est par ici — dépense, puis repars » (accent
  `FORGE_BRAISE`, l'appel à l'action)

Vocabulaire CE2, ≤ 8 mots par ligne, conforme à la bible cartoon des lignes voisines.

### 4. 🔴 Les Âmes gagnées en run ne se perdent plus sur un alt-F4

**Le défaut.** `sys_flush_meta_save` n'était câblé que sur `OnExit(GameMode::Roguelite)`,
`OnEnter(Victory)` et `OnEnter(Defeat)` (`meta_shop.rs:922-924`) — **trois sorties
propres**. Les Âmes gagnées pendant la run (5/vague, 25/boss, 2/wisp, 10/pièce et
25/étoile du parcours — soit **le revenu entier d'une run, ~63 Âmes**) ne vivaient que
dans la Resource `MetaSouls`. Fermer le jeu en pleine run perdait tout.

**Le correctif.** `sys_autosave_meta_souls`, `run_if(in_state(GameMode::Roguelite))` :

- écrit **uniquement si le total a bougé** (`save.souls_total != meta.current`) → zéro I/O
  quand rien ne change, et pas de churn de change-detection sur `MetaShopSave` ;
- au plus une fois toutes les `AUTOSAVE_INTERVAL_SECS` (10 s) → perte maximale bornée à
  une poignée d'Âmes ;
- `Time<Real>` et non `Time<Virtual>` : un autosave ne doit pas être gelé par une pause
  (cf anti-trap « `Time<Real>` vs `Time<Virtual>` » du CLAUDE.md).

> `AUTOSAVE_INTERVAL_SECS` est un `const` nommé **assumé** : c'est de la plomberie de
> persistance, pas un levier de gameplay — même statut que `SAVE_VERSION` / `SAVE_FILE`
> juste au-dessus, et explicitement hors du périmètre exposable au créateur
> (`creator-simplicity.md` : « seuils de performance / détails d'implémentation »).

---

---

## Passe d'auto-QA adverse (obligatoire, `post-impl-auto-qa.md`)

3 angles de revue indépendants (correctness/régression · data/genome/conventions ·
UX/observabilité), puis **un réfutateur par finding** chargé de prouver que le finding est
faux. **6 défauts ont survécu à la réfutation — dont 2 régressions introduites par le
correctif lui-même.** Tous corrigés dans la même livraison.

### 🔴 Corrigé — le plafond détruisait la progression au lieu de la borner

Le réfutateur a **lu le save réel de la machine** : `%APPDATA%\Forgia\meta_shop_save.toml`
contient `[weapon_levels] pepin = 13`. Avec le clamp posé **à l'écriture**, la fin de la
run suivante aurait calculé `before = 13`, `level = min(14, 6) = 6`, donc `level != before`
→ `save.save()` écrivait **13 → 6 sur le disque**, définitivement, en loggant
« pepin → niveau 6/6 » comme s'il s'agissait d'un gain. Et l'écran de choix d'arme
affichait entre-temps « Niveau 13/6 (+20 % dégâts) ».

Le clamp à l'écriture n'apportait **rien** : `damage_mul` bornait déjà à la lecture.

**Correction** : `level_up_weapon` n'incrémente que si `*lvl < cap` et **ne fait jamais
redescendre** une valeur existante. L'UI affiche `MasteryConfig::effective_level(stored)`.
Bénéfice de bord : relever `max_level` en genome plus tard **rendra** leur progression aux
joueurs au lieu de l'avoir amputée.

### 🔴 Corrigé — le reset des atouts ouvrait une faille dans le gating des légendaires

**Régression créée par le correctif n° 1.** `reset_run()` vide `unlocked_legendary`, mais
rien ne remettait `CoffreSession` à zéro. Scénario reproductible : run 1, j'accumule 3 tags
→ un légendaire devient éligible → le Coffre s'ouvre et le roule dans `candidates` → je ne
clique pas, je meurs pendant le break ou j'ESC. Run 2 : `unlocked_legendary` est vide, mais
la modale est toujours ouverte avec le légendaire dedans — et `sys_handle_coffre_pick` ne
revérifie **jamais** l'éligibilité au moment du pick. Légendaire obtenu sans avoir atteint
le moindre seuil. **Avant ce correctif le bug n'existait pas**, puisque `unlocked_legendary`
survivait à la run.

**Correction** : `CoffreSession` est purgée dans la **même closure exclusive** que
`ActiveBoons` — les deux forment un seul état de run, elles se remettent à zéro ensemble.

### 🔴 Corrigé — une section `[mastery]` partielle tuait tout le catalogue, en silence

`MasteryToml` n'avait pas de `#[serde(default)]` sur ses champs : le `#[serde(default)]`
posé sur `mastery: Option<...>` ne couvre que l'**absence** de la section, pas une section
**incomplète**. Un créateur écrivant `[mastery]\nmax_level = 8` (« je garde le bonus par
défaut ») faisait échouer serde sur le **document entier** → upgrades, prix d'armes et prix
de paliers retombaient tous sur le miroir Rust. Et le fallback était **muet** : le log de
succès imprime `upgrades=4`, ce que le miroir Rust produit aussi — indiscernable.

**Correction** : `#[serde(default)]` sur les deux champs + un `warn!` explicite sur les
**deux** chemins de fallback (parse KO et fichier introuvable).

### 🔴 Corrigé — le genome pouvait produire un multiplicateur absurde ou négatif

La seule validation était `max_level >= 1`. Le commentaire du TOML parlait en **pourcents**
alors que la valeur est une **fraction** : un créateur voulant 10 % écrit `10`, serde
l'accepte, et `damage_mul(6)` renvoie **×51 de dégâts permanents**. Symétriquement `-0.5`
donnait un multiplicateur **négatif** propagé à la chaîne de combat. Violation directe de
`genome-code.md` (« chaque gene a une valeur min/max raisonnable ») et de
`creator-simplicity.md` (« un créateur ne doit jamais casser son jeu »).

**Correction** : `MasteryConfig::from_genome` borne `max_level` à `1..=20` et
`damage_per_level` à `0.0..=0.25` (NaN neutralisé), **avec un `warn!` quand la valeur a été
corrigée** — une correction silencieuse est aussi opaque que le défaut qu'elle répare. Le
commentaire du TOML dit maintenant l'unité.

### 🟡 Corrigé — l'autosave persistait aussi les DÉPENSES

`sys_autosave_meta_souls` écrivait dans les deux sens. Or le seul débit d'Âmes en run est
le « Second souffle » du marchand, dont la contrepartie — le jeton de revive — est un état
de run **non persisté**. Acheter puis quitter 12 s plus tard perdait **les Âmes ET
l'objet**. Avant le correctif, quitter annulait la dépense en même temps que les gains.

**Correction** : autosave **à la hausse uniquement**. Le solde exact reste scellé par
`sys_flush_meta_save` en fin de run.

### 🟡 Corrigé — l'invariant n'était observable nulle part + pas de hot-reload

- `forgia2_roguelite_state.json` expose désormais `weapon_levels` et `mastery_cap`, plus un
  `severity: "info"` quand un niveau stocké dépasse le plafond (cas normal d'un save
  antérieur — le dire évite de re-diagnostiquer « le plafond ne marche pas »).
  `xtask sensor-audit` : **124/124, 0 orphelin, 0 manquant**.
- `sys_hot_reload_meta_shop_genome` (poll mtime 1 Hz, patron de `ultimate_config.rs`) :
  `genome-code.md` exige que **tout** gène marche en hot-reload, et `[mastery]` est
  précisément celui destiné à être itéré en passe de balance.

### 🟡 Corrigé — formulations FR et commentaire faux

- « repart » / « repars » sont homophones : sans sujet, la ligne se lisait comme
  l'impératif « repars tout de suite ! », soit **l'inverse** de l'avertissement. Repassé en
  2ᵉ personne, une idée par ligne : « ↑ tu repars sans rien acheter » et « ↑ passe par
  L'Enclume, dépense tes Âmes ».
- Le commentaire justifiant le `commands.queue` invoquait une garde `is_changed` de
  `sys_recompute_boon_mods` **retirée le 2026-06-28**. Le reset fonctionne, mais pas pour
  la raison écrite — corrigé par la vraie (recompute inconditionnel en `GameSet::Effects`,
  après le sync point qui suit `GameSet::Movement`).

### 🔵 Signalé, PAS corrigé — ce sont des décisions de design, pas des correctifs

1. **Hiérarchie visuelle des boutons Defeat.** Les flèches pointent le bon bouton, mais
   REJOUER garde le glyphe `↻` et `FORGE_OR` (le CTA principal de la DA) tandis que le
   chemin recommandé porte `✕` et une couleur secondaire. Un enfant balaie l'écran et
   appuie sur le gros bouton doré. Inverser l'emphase quand `first_death` est un **choix
   de DA**, pas un correctif.
2. **L'écran Victoire porte le même piège**, sans aucun panneau : son REJOUER fait le même
   `next_run.set(RunState::Lobby)`, et c'est le moment où le joueur a le plus d'Âmes de
   toute la session.
3. **Valeur du plafond** : le GDD M5 spécifie `cap 10, +2 %/niveau` (+18 %) ; le livré est
   `6 × 4 %` (+20 %). J'ai gardé `damage_per_level = 0.04` pour ne pas modifier le gain par
   niveau des saves existantes — changer les deux à la fois aurait mélangé « poser un
   plafond » et « refaire la courbe ». **À trancher.** ROADMAP annotée.

---

## Fichiers touchés

| Fichier | Nature |
|---|---|
| `assets/genomes/roguelite/roguelite_meta_shop.toml` | **definition** — section `[mastery]` bornée + unité documentée |
| `crates/forgia-mode-roguelite/src/meta_shop.rs` | `MasteryConfig` (bornes, `effective_level`), parsing non destructif, autosave, hot-reload |
| `crates/forgia-mode-roguelite/src/weapon_select.rs` | `const` retiré → genome, 3 sites unifiés, niveau affiché borné |
| `crates/forgia-mode-roguelite/src/run.rs` | reset `ActiveBoons` **+ `CoffreSession`** au run-start |
| `crates/forgia-mode-roguelite/src/hud.rs` | flèches FTUE de l'écran Defeat |
| `crates/forgia-mode-roguelite/src/sensor.rs` | `weapon_levels` + `mastery_cap` + health check |
| `docs/observability/SENSOR_REGISTRY.md`, `docs/ROADMAP.md` | doc à jour (item ROADMAP barré) |

---

## Critères d'acceptation

- [x] `ActiveBoons::reset_run()` appelé au run-start, sans dépasser le plafond de 16 SystemParams
- [x] Plafond de maîtrise **data-driven** (`[mastery]` du genome) + miroir Rust exact
- [x] Le `const WEAPON_MASTERY_DMG_PER_LEVEL` n'existe plus ; les 3 sites lisent la même source
- [x] L'UI affiche `Niveau x/cap` — l'affiché ne peut plus diverger du runtime
- [x] Flèches FTUE sous le bon bouton, vocabulaire CE2 ≤ 8 mots
- [x] Autosave des Âmes en run, sans I/O inutile, `Time<Real>`
- [x] **Aucune donnée de save n'est jamais rabaissée** (clamp à la lecture, pas à l'écriture)
- [x] `CoffreSession` purgée avec `ActiveBoons` (pas de faille dans le gating des légendaires)
- [x] Une section `[mastery]` partielle ne tue plus le document, et tout fallback **loggue**
- [x] Valeurs du genome **bornées** (`max_level` 1..=20, `damage_per_level` 0..=0.25, NaN neutralisé)
- [x] Autosave **à la hausse uniquement** (une dépense n'est pas scellée sans sa contrepartie)
- [x] Hot-reload du genome (`genome-code.md` : « tout gene DOIT fonctionner avec Shift+F12 »)
- [x] Invariant **observable** : `weapon_levels` + `mastery_cap` + health check dans le capteur
- [x] `cargo check --workspace` vert
- [x] `cargo clippy -p forgia-mode-roguelite --all-targets -- -D warnings` vert (0 warning)
- [x] `cargo test -p forgia-mode-roguelite --lib` : **284 passed, 0 failed** (dont 7 nouveaux)
- [x] `cargo run -p xtask -- sensor-audit` : **124/124, 0 orphelin, 0 manquant**
- [x] Le TOML parse encore ses 4 upgrades / 3 armes / 3 paliers après l'ajout de `[mastery]` (vérifié par `tomllib`)
- [ ] **Validation runtime** (cf récap ci-dessous)

## Tests ajoutés (7)

| Test | Ce qu'il prouve |
|---|---|
| `mastery_damage_mul_grows_then_stops_at_cap` | niveau 1 = ×1,0 · niveau 6 = ×1,20 · niveau 50 = niveau 6 |
| `level_up_weapon_stops_at_cap` | 20 runs consécutives ne dépassent pas le plafond |
| `level_up_weapon_returns_unchanged_level_at_cap` | au plafond le niveau retourné est stable → pas d'écriture disque |
| **`level_up_weapon_never_lowers_a_legacy_level`** | **régression QA** : un save à 13 reste à 13 sur le disque, le bonus est borné à la lecture, l'UI affiche 6/6 |
| `mastery_comes_from_the_genome_and_falls_back_when_absent` | le genome pilote · absent → miroir Rust |
| **`a_partial_mastery_section_does_not_kill_the_whole_document`** | **régression QA** : `[mastery]` incomplète → le champ manquant prend son défaut, les upgrades et les sections suivantes survivent |
| **`genome_values_are_bounded_so_a_creator_cannot_break_the_game`** | `1000` → 20 · `10.0` → 0,25 · `-0.5` → 0,0 (jamais de multiplicateur négatif) · NaN → défaut |

---

## Ce que cette story ne fait PAS (assumé, hors scope)

Le rapport classe 18 autres ruptures. Les plus structurantes restent ouvertes et relèvent
de la **conception**, pas du correctif :

- `room_kind` (choix de porte) toujours écrit et jamais relu → le choix reste cosmétique
- `wave_composition(wave)` toujours sans `stage` / `kind` / `seed`, `WAVE_BASE_SEED` constant
- `difficulty_budget` toujours calculé puis jeté
- difficulté toujours **100 % statistique**
- `ElementUnlocks` toujours reset sur `OnEnter(GameMode::Roguelite)`, qui ne tire pas entre
  deux runs (sans effet tant que `always_on = true`)
- aucune méta-méta (ascension / heat / NG+)

À planifier en Vagues 1-3 du rapport.

---

## Récap de test runtime

1. **Action** : lancer le Roguelite, **mourir volontairement** en salle 1 après avoir pris
   2-3 atouts au Coffre, cliquer **REJOUER**, puis regarder le bandeau d'atouts du HUD.
2. **Rechargement** : `cargo run -p forgia` (rebuild nécessaire — c'est du Rust).
   ⚠️ Le binaire est `forgia.exe` (paquet `forgia`), **pas** `-p forgia-game`.
3. **Effet attendu** :
   - le bandeau d'atouts est **VIDE** au début de la run 2 (il était plein avant le fix) ;
   - le log affiche `[roguelite] sys_start_run — N atouts purgés (nouvelle run)` ;
   - au menu → onglet ARMES, la carte affiche `Niveau x/6` et le bonus plafonne à +20 % ;
   - à la 1ʳᵉ mort, la flèche braise « L'Enclume est par ici » est **sous RETOUR AU MENU**,
     et la ligne crème « repart tout de suite, sans rien acheter » sous REJOUER ;
   - fermer le jeu (alt-F4) **en pleine run** puis relancer : les Âmes gagnées avant la
     dernière tranche de 10 s sont **conservées**.
4. **Où observer** — le capteur est la référence, pas le log :
   - **`forgia2_boons.json`** (`forgia-observability/src/boons_sensor.rs:32`) →
     `active_count` doit retomber à **0** au début de la run 2, et `unlocked_legendary`
     à **0** aussi. `total_choices` continue de monter : c'est **voulu**, ce compteur est
     cumulatif de session par conception (`reset_run()` le préserve explicitement) ;
   - **`forgia2_roguelite_state.json`** → `weapon_levels` et `mastery_cap` (nouveaux) ;
     `severity: "info"` si un niveau stocké dépasse le plafond — c'est **normal** sur un
     save antérieur, le bonus est borné à la lecture ;
     et `souls_persistent` vs `meta_souls_total` doivent **converger en ≤ 10 s** pendant
     la run (avant le correctif ils divergeaient toute la run) : c'est la preuve directe
     que l'autosave tourne ;
   - log console : `[roguelite] sys_start_run — N atouts purgés (nouvelle run)` ;
   - bandeau d'atouts du HUD in-game ;
   - `%APPDATA%\Forgia\meta_shop_save.toml` (`souls_total`, `weapon_levels`) pour
     l'autosave et le plafond.
5. **Variantes si KO** :
   - bandeau encore plein en run 2 → vérifier que le log `atouts purgés` sort ; s'il sort
     mais que le HUD reste plein, le lecteur du HUD lit autre chose que `ActiveBoons` ;
   - `Niveau x/6` absent → le TOML n'est pas trouvé (chemin **relatif au CWD**) : lancer
     depuis la racine du repo, ou vérifier le fallback miroir Rust (même valeurs) ;
   - Âmes toujours perdues à l'alt-F4 → vérifier que la run a duré **> 10 s** depuis le
     dernier gain (l'autosave est throttlé) ;
   - aucun effet du tout → **exe périmé** : comparer `stat` de `target/*/forgia.exe` et de
     `crates/forgia-mode-roguelite/src/run.rs` avant de conclure quoi que ce soit.
