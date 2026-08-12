# story-702 — `forgia-mode-roguelite` : le risque n'est pas la taille, c'est le code sans filet

**Statut** : DRAFT — analyse faite, aucun découpage commencé
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (tests d'abord, découpe mécanique ensuite ; 1 crate)
**Origine** : question d'Antoine le 2026-08-12 — *« forgia-mode-roguelite n'aurait pas des
fichiers avec trop de LOC qui mériteraient d'être splittés ? »*
**Jumelle** : [story-701](story-701-decoupe-forgia-stage-lib.md) — même défaut d'instrument,
même correctif partagé.

---

## La mesure, code et tests séparés

| Fichier | **Code** | Tests | Ratio | Verdict |
| --- | --- | --- | --- | --- |
| **hud.rs** | **2 392** | 278 | 0,12 | 🔴 le vrai god-file |
| decor.rs | 2 011 | 542 | 0,27 | correctement testé |
| elements.rs | 1 636 | 700 | 0,43 | ✅ |
| audio.rs | 1 343 | 233 | 0,17 | 🟠 |
| run.rs | 1 142 | 206 | 0,18 | 🟠 |
| equipment.rs | 1 093 | 149 | 0,14 | 🟠 |
| **loot_room.rs** | **1 077** | **0** | **0** | 🔴 **aveugle** |
| waves.rs | 1 032 | 792 | 0,77 | ✅ |
| lib.rs | 938 | 8 | — | orchestrateur, taille saine |
| weapon_select.rs | 933 | 97 | 0,10 | 🟠 |
| **meta_shop.rs** | **734** | **1 518** | **2,07** | ✅ **le mieux testé de la crate** |
| **status_vfx.rs** | **668** | **0** | **0** | 🔴 **aveugle** |

## L'instrument se trompe de cible, et c'est le vrai sujet

`meta_shop.rs` figurait **7ᵉ god-file du projet à 2 253 lignes brutes**. En réalité :
**734 lignes de code pour 1 518 de tests** — le fichier le mieux couvert de la crate.
**Le découper aurait été une pure régression.**

La détection du projet ([`fine-grained-crates.md`](../../.claude/rules/fine-grained-crates.md) §6)
est `wc -l > 1200`, qui **ne sépare pas le code des tests**. Elle accuse donc les fichiers
*bien testés* et laisse passer ceux qui n'ont aucun filet. C'est la même classe que
[story-699](story-699-un-capteur-a-zero-ne-doit-pas-dire-ok.md) : **l'instrument ment**, et
il a orienté la question de départ vers la mauvaise cible.

## Le reclassement qui compte

> Un fichier de 2 000 lignes avec 800 tests est **maintenable** : on peut le découper, les
> tests disent si on a cassé quelque chose. Un fichier de 1 077 lignes **sans un seul test**
> ne peut être ni vérifié, ni découpé en sécurité.

D'où la règle d'ordre de cette story :

**On ne découpe pas un fichier non testé. On le teste d'abord.** Découper sans filet est
précisément la manière d'introduire une régression silencieuse dans un chantier qu'on
croyait mécanique.

## Le plan, dans cet ordre

### 1. `loot_room.rs` — 1 077 lignes, zéro test 🔴

**Le plus urgent, et ce n'est pas un découpage.** 1 077 lignes aveugles dans la boucle de
loot — c'est-à-dire dans l'une des trois chasses du [GDD](../design/gdd-forgia-the-spared.md).
Écrire les tests des fonctions pures d'abord ; le découpage devient une question ensuite,
peut-être inutile.

### 2. `status_vfx.rs` — 668 lignes, zéro test 🔴

Même traitement, moindre volume. Déjà cité dans la dette technique de la ROADMAP
(« split des hotspots ») — mais le split n'était pas le bon premier geste.

### 3. `hud.rs` — 2 392 lignes de code 🔴

**Là, le découpage mécanique est le bon geste** : il a un minimum de tests, et la recette
`reference_decoupe_mecanique_god_file` s'applique (sed bas→haut, en-tête générique,
`cargo fix`, puis preuve que le câblage est identique à HEAD).

⚠️ `hud.rs` est aussi la cible de l'**inc. 4d** de [story-700](story-700-navmesh-fondation-compagnon.md)
(barre PV du compagnon). Faire la découpe **avant** évite d'ajouter au god-file ; la faire
**après** évite un conflit. À coordonner, pas à improviser.

### 4. Corriger l'instrument (partagé avec [story-701](story-701-decoupe-forgia-stage-lib.md))

La détection de god-files doit classer par **code non testé**, pas par lignes brutes.
Sans ça, la prochaine session refera exactement l'erreur d'aiguillage de celle-ci.

## Critères d'acceptation

- [ ] `loot_room.rs` a des tests sur ses fonctions pures — **avant tout découpage**
- [ ] `status_vfx.rs` idem
- [ ] `hud.rs` ≤ 1 400 lignes de code, modules avec en-tête disant **ce qu'ils ne font pas**
- [ ] `cargo test -p forgia-mode-roguelite` — au moins autant de tests qu'avant, tous verts
- [ ] `cargo clippy -p forgia-mode-roguelite --all-targets` — 0 warning (**vrai cargo**, RTK masque)
- [ ] Preuve de non-régression sur `hud.rs` : le diff ne montre que des déplacements
- [ ] La détection de god-files sépare code et tests
- [ ] **`meta_shop.rs` non modifié** — c'est un critère, pas un oubli

## Ce que cette story ne fait PAS

- **Toucher `meta_shop.rs`** — 2,07 de ratio, il va très bien
- **Toucher `waves.rs` ni `elements.rs`** — bien testés
- **Toucher `decor.rs`** — modifié le 2026-08-12 (correctif de spawn), correctement testé
- **Éclater la crate** — question distincte, non instruite ici

## Priorité

**Moyenne pour `loot_room.rs`** (1 077 lignes aveugles dans une boucle de gameplay livrée),
**basse pour le reste**. Aucune phase de [la refonte](../REFONTE_GDD.md) n'est bloquée.

⚠️ `forgia-mode-roguelite` est l'arbre chaud de l'autre terminal : **coordonner avant de
démarrer** (`multi-terminal-coordination.md` §3).
