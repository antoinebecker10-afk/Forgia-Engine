# Story-613 — Roguelite : déblocage permanent des armes (progression « évolutive »)

> **Statut** : CODE-COMPLETE (2026-06-23) — validation runtime à faire
> **Niveau BMAD** : Standard (genome + `meta_shop.rs` + `weapon_select.rs`)
> **Demande user** : « faut limiter les armes, atouts etc. pour rendre le jeu
> évolutif, fais-moi des propositions ».
> **Rapport/audit** : [best-practices-wizard-roguelite-2026-06-23](../best-practices-wizard-roguelite-2026-06-23.md) · prérequis [story-612](story-612-roguelite-weapon-select-wizard.md).

## Décisions design (AskUserQuestion 2026-06-23)

- **Modèle d'unlock** = **Âmes à l'Enclume** (réutilise `MetaShopSave`, persistance disque) — modèle Hadès/Gunfire.
- **Départ** = **minimal** : Pépin seul + (à terme) boons Common. Le wizard grandit de 1 à 4 cartes.

## Scope — Increment 1 : déblocage des ARMES

Le plus visible et le plus contenu (forgia-mode-roguelite uniquement). Le gating
des **paliers de boons** (cross-crate `forgia-rpg-data`) = increment 2, hors scope ici.

| Arme | Clé genome | Coût (Âmes) | Élément |
|---|---|---|---|
| Pépin | `pepin` | **gratuite** (départ, jamais listée) | Explosif |
| Bourrasque | `bourrasque` | 60 | Feu |
| Madame Lenoir | `madame_lenoir` | 150 | Perforant |
| Boucherie | `boucherie` | 250 | Poison |

Coûts croissants = courbe évolutive. Couplage gratuit : débloquer une arme débloque
son **élément** de départ (l'élément armé suit `EquippedWeapons.current`).

## Architecture (concept-first)

- Concept = `combat` / progression (couche fw + def TOML). Net = local. Script = interne.
- **Producteur (vérité)** :
  - coûts = `[[weapon_unlocks]]` dans `assets/genomes/roguelite/roguelite_meta_shop.toml`
    → `MetaShopCatalogue.weapon_unlocks` (miroir Default).
  - possession = `MetaShopSave.unlocked_weapons: Vec<String>` (persisté `config/meta_shop_save.toml`,
    défaut = `["pepin"]` via `#[serde(default)]` → **un save d'avant 613 retombe sur Pépin seul**).
- **Consommateurs** :
  - `weapon_select::sys_weapon_unlock_input` (frame, `GameSet::UI`, run_if Lobby) — `[U]` débloque
    l'arme sélectionnée si verrouillée + assez d'Âmes (mute `MetaSouls` + `MetaShopSave`, save disque).
  - `weapon_select::draw_weapon_select` — carte grisée + « ◈ VERROUILLÉE — N Âmes » + footer `[U]` ;
    compteur Âmes ; strip marque les verrouillées d'un `*`.
  - `weapon_select::sys_apply_weapon_choice` — **clamp** : jamais démarrer avec une arme verrouillée
    (fallback Pépin).
- **Persistance** : réutilise `MetaShopSave::save()` (atomique, `config/meta_shop_save.toml`).

## Critères d'acceptation

- [ ] Nouveau save : seul Pépin jouable ; Bourrasque/Lenoir/Boucherie affichées **verrouillées** + coût.
- [ ] `←/→` parcourent les 4 cartes (verrouillées incluses, en teaser grisé).
- [ ] `[U]` sur une arme verrouillée + assez d'Âmes → débloque (Âmes débitées, persisté disque).
- [ ] `[U]` sans assez d'Âmes → no-op + log (footer « N Âmes manquantes »).
- [ ] `ENTRÉE` démarre toujours avec une arme **possédée** (clamp Pépin si sélection verrouillée).
- [ ] Coûts lus du genome `roguelite_meta_shop.toml` (fallback miroir par champ).
- [x] `cargo check` + clippy 0 warning (sur la crate) + **19 tests** verts (11 meta_shop + 8 weapon_select) + binaire `cargo build -p forgia -j 4` OK.

## Auto-QA (post-impl, 2026-06-23)

- **Mécanique** : check + clippy + 19 tests + build binaire — verts (cf AC).
- **Invariants vérifiés** : backward-compat save (`old_save_without_field_defaults_to_pepin`),
  idempotence unlock, round-trip persistance, Pépin jamais déblocable (gratuit). Pas de conflit
  clavier (`U` ≠ `1-4`/ENTRÉE meta-shop ≠ `←/→` wizard). Double-ResMut MetaSouls/MetaShopSave =
  sérialisé par Bevy (pas de panic).
- ⚠️ **Ton save actuel** (`config/meta_shop_save.toml`, ~1666 Âmes) → au prochain lancement, seul
  Pépin sera débloqué ; tu re-débloques les 3 autres instantanément (60+150+250 = 460 < 1666).
  C'est le comportement voulu (pour un nouveau joueur c'est une vraie progression).

## Suivi (hors scope increment 1)

- **Increment 2** : gating des **paliers de boons** (Common gratuit → Uncommon/Rare/Légendaire
  déblocables). Cross-crate : `forgia-rpg-data::boons::roll_candidates` doit filtrer par palier
  débloqué (passer un `UnlockedBoonTiers` en Resource depuis forgia-mode-roguelite).
- **Increment 3** : déblocages par **accomplissement** (battre le boss, X kills à l'arme Y…) —
  plus gratifiant pour les vétérans (ton save à 1666 Âmes débloque tout direct sinon).
- ENTRÉE sur une arme verrouillée = clamp Pépin (v1) ; idéalement → invite « débloque d'abord ».
- Héros (modèle Gunfire) quand 2+ persos existeront.
- Externaliser les taglines persona en genome (v1 = texte UI local).
