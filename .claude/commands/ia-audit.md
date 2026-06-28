# Pipeline Audit Codebase Forgia

Tu es maintenant en mode **Audit Technique Profond**. Active les 4 agents specialises en parallele.

Le code source est dans `D:/Forgia/RUST/Forgia/Forgia/src/`.

---

## Agent 1 : Architect — Structure & Couplage

Analyse l'architecture du code source Rust :

1. **God Modules** : fichiers >500 lignes ou >15 types publics (`pub struct`, `pub enum`, `pub fn`). Lister chaque fichier avec son nombre de lignes et de types publics.
2. **Modules Orphelins** : fichiers `.rs` presents dans `src/` mais jamais references dans un `mod.rs`, `main.rs` ou `lib.rs`.
3. **Couplage inter-modules** : pour chaque module (ai, combat, debug, effects, gamemode, inventory, network, persistence, player, sky, terrain, triggers, ui, world), compter les `use crate::` imports entrants et sortants. Identifier les dependances circulaires.
4. **God Functions** : fonctions >80 lignes. Lister fichier:ligne + nom + nombre de lignes.
5. **SystemParam manquants** : systemes Bevy avec >12 parametres qui n'utilisent pas de `SystemParam` bundle.

Pour chaque probleme :
- **Fichier** : chemin relatif depuis src/
- **Severite** : Critique / Haute / Moyenne / Basse
- **Suggestion** : action concrete (split, extract, decouple)

---

## Agent 2 : Dev Senior — Code Mort, Doublons & Hardcoding

Analyse le code source pour les dettes techniques :

1. **Code Mort** : fonctions `pub` ou `pub(crate)` jamais appelees ailleurs dans le codebase (grep le nom de la fonction, si 1 seul match = declaration uniquement = mort). Structs/enums jamais instancies ou references.
2. **Doublons** : blocs de code >10 lignes quasi-identiques entre fichiers differents. Patterns repetes qui pourraient etre factorise.
3. **Hardcoding** :
   - Nombres magiques (valeurs numeriques litterales hors 0, 1, 2, -1, 0.0, 1.0, PI, TAU)
   - Chemins de fichiers en dur (strings contenant `/`, `\\`, `.glb`, `.gltf`, `.png`, `.wav`, `.mp3`, `.ogg`)
   - Strings repetees identiques dans 3+ endroits
4. **TODO/FIXME/HACK** : lister tous les commentaires contenant ces mots-cles avec fichier:ligne et le texte du commentaire.
5. **Imports inutilises** : `use` statements dont le symbole importe n'est jamais utilise dans le fichier.

Pour chaque probleme :
- **Fichier:ligne** : localisation exacte
- **Type** : Dead / Doublon / Hardcode / Debt / Import
- **Impact** : taille du code concerne ou frequence de repetition

---

## Agent 3 : QA & Security — Robustesse & Securite

Analyse le code pour les risques de stabilite :

1. **`unwrap()` dangereux** : tous les `.unwrap()` sur des operations I/O, reseau (`reqwest`), parsing (`serde_json`, `toml`), `asset_server.load()`. Exclure les unwrap sur des `.get()` avec check prealable ou des `expect()` avec message.
2. **`panic!` explicites** : tous les `panic!()`, `unreachable!()`, `unimplemented!()` en dehors de code de debug.
3. **`unsafe` blocks** : lister tous les blocs unsafe avec justification si commentee.
4. **Gestion d'erreur** : fonctions qui retournent `Result` mais dont les appelants ignorent l'erreur (`. ok()`, `let _ =`, `drop()`).
5. **Stability Locks** : verifier chaque LOCK L1-L8 defini dans CLAUDE.md :
   - L1 : chercher `asset_server.load()` hors exceptions documentees (AudioRegistry, NexusForge, vegetation/village/enemy scene_cache)
   - L2 : PerfMode toggle F4 present et fonctionnel
   - L3 : CameraCollisionCache avec timer 33ms
   - L4 : EditorRaycastResult centralise
   - L5 : NameplateCache 10Hz
   - L6 : toggle_editor_effects avec run_if resource_changed
   - L7 : GameSet chain Input->Movement->Physics->Camera->Combat->Effects->UI
   - L8 : MinimapCache avec seuil mouvement

Pour chaque probleme :
- **Fichier:ligne** : localisation
- **Risque** : Critique (crash prod) / Haut (bug silencieux) / Moyen (edge case) / Bas (cosmetic)
- **Fix** : action concrete

---

## Agent 4 : Perf Engineer — Performance ECS & Runtime

Analyse les patterns de performance :

1. **Systemes sans `run_if`** : systemes enregistres dans l'app sans condition `.run_if()`. Lister le nom du systeme et le plugin qui l'enregistre.
2. **Allocations hot path** : `Vec::new()`, `String::new()`, `format!()`, `HashMap::new()` dans des systemes qui tournent chaque frame (pas de `run_if` ou `run_if` toujours vrai). Suggerer pre-allocation ou `Local<>`.
3. **Queries trop larges** : `Query<>` qui fetch >6 composants sans filtre `With<>`/`Without<>`.
4. **Raycasts non caches** : appels raycast (`cast_ray`, `cast_shape`) en dehors d'un pattern cache (timer + Local).
5. **Events non draines** : `EventReader<>` declares mais jamais iteres, ou `EventWriter<>` qui envoient sans lecteur.
6. **Assets recharges** : `asset_server.load()` appele chaque frame au lieu d'une seule fois (pas de garde `if handle.is_none()`).
7. **Clone/Copy abusifs** : `.clone()` sur des types lourds (Vec, String, HashMap) dans des systemes par-frame.

Pour chaque probleme :
- **Systeme** : nom + fichier
- **Impact perf** : Critique (frame drop) / Haut (CPU waste) / Moyen (allocation pressure) / Bas (micro-opt)
- **Fix** : pattern correct a utiliser

---

## Output attendu

Genere un rapport structure complet :

```
# RAPPORT AUDIT FORGIA — {YYYY-MM-DD}

## Resume Executif
- Score global : X/100 (calcule : -5 par critique, -3 par haut, -1 par moyen, depuis 100)
- Total problemes : N (C critiques, H hauts, M moyens, B bas)
- Top 3 actions prioritaires

## 1. Architecture & Couplage (Architect)
| # | Fichier | Probleme | Severite | Suggestion |
|---|---------|----------|----------|------------|

### Matrice de couplage
| Module | Imports IN | Imports OUT | Circulaire |
|--------|-----------|------------|------------|

## 2. Code Mort, Doublons & Hardcoding (Dev Senior)
| # | Fichier:ligne | Type | Description | Impact |
|---|--------------|------|-------------|--------|

### TODO/FIXME Tracker
| # | Fichier:ligne | Texte | Age estimee |
|---|--------------|-------|-------------|

## 3. Securite & Stabilite (QA)
| # | Fichier:ligne | Risque | Description | Fix |
|---|--------------|--------|-------------|-----|

### Stability Locks Status
| Lock | Status | Detail |
|------|--------|--------|
| L1   | OK/VIOLATION | ... |
| ...  | ...    | ... |

## 4. Performance ECS (Perf)
| # | Systeme | Fichier | Impact | Fix |
|---|---------|---------|--------|-----|

## 5. Recommandations (Top 10 prioritaires)
1. [CRITIQUE] ...
2. [CRITIQUE] ...
3. [HAUTE] ...
...

## 6. Metriques Codebase
- Fichiers .rs : N
- Lignes totales : N
- Fichiers >500 lignes : liste
- Fonctions >80 lignes : N
- unwrap() count : N
- TODO/FIXME count : N
- Systemes sans run_if : N
```

Sauvegarde le rapport dans `docs/audits/audit-{YYYY-MM-DD}.md`.

Si le dossier `docs/audits/` n'existe pas, le creer.

Apres avoir genere le rapport, affiche un resume executif a l'utilisateur avec les 5 actions les plus critiques.
