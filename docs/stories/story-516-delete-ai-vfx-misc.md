# Story-516 — DELETE vague AI + VFX + Misc (20 crates)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (fichier `lib.rs`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : WIP
**BMAD Scale** : Standard (~20 fichiers crate dirs supprimés, bounded scope, suppression sèche)
**Created** : 2026-05-26
**Branch** : `cleanup/story-516-delete-ai-vfx-misc`
**Predecessor** : story-512 + story-513 (DONE, PR #1 merged 2026-05-26)
**Source plan** : `docs/audit/scaffolds-audit-2026-05-23.md`

---

## 1. Contexte

Suite cleanup vagues 1-4 (PR #1 mergée). État après merge :
- 211 crates effectivement membres workspace (Cargo.toml), 165 dirs disque post-cleanup
- 93 scaffolds <50 LOC restants identifiés par audit 2026-05-23
- Ratio 347 LOC/crate vs cible industrie 1500-5000 (4× sous cible)

**Cette story = vague suivante de suppression sèche**, 20 crates 100% scaffolds 16 LOC, **0 consumers vérifié** (`grep workspace.dependencies` 2026-05-26).

---

## 2. Scope exact — 20 crates

### Cluster A : AI Subsystems (9 crates, 0 consumers)
| Crate | LOC | Justif |
|---|---|---|
| forgia-ai-blackboard | 16 | Plugin stub, 0 consumer |
| forgia-ai-bt | 16 | idem |
| forgia-ai-flocking | 16 | idem |
| forgia-ai-formation | 16 | idem |
| forgia-ai-goap | 16 | idem |
| forgia-ai-navmesh | 16 | idem |
| forgia-ai-perception | 16 | idem |
| forgia-ai-state-machine | 16 | idem |
| forgia-ai-utility | 16 | idem |

**Garder** : `forgia-ai-arena-bot` (868 LOC réel, used Roguelite).

### Cluster B : VFX scaffolds (3 crates, 0 consumers)
| Crate | LOC | Justif |
|---|---|---|
| forgia-vfx-decals | 16 | Plugin stub |
| forgia-vfx-hanabi | 16 | idem |
| forgia-vfx-impact-library | 16 | idem |

**Garder** : `forgia-vfx-tracers` (98 LOC, implem partielle).

### Cluster C : Misc scaffolds (7 crates, 0 consumers)
| Crate | LOC | Justif |
|---|---|---|
| forgia-anticheat | 16 | scaffold |
| forgia-cloth | 16 | scaffold |
| forgia-marketplace-client | 16 | scaffold |
| forgia-oxr | 16 | scaffold OpenXR |
| forgia-ragdoll | 16 | scaffold |
| forgia-scripting-luau | 16 | scaffold |
| forgia-steam | 16 | scaffold |

### Cluster D : Scaffold orphelin cassé (1 crate)
| Crate | LOC | Justif |
|---|---|---|
| forgia-mod-outline | 16 | Casse `cargo test` (bevy_mod_outline 0.12 vs Bevy 0.18). 0 consumer. |

**Total** : 9 + 3 + 7 + 1 = **20 crates** à supprimer.

---

## 3. Acceptance Criteria

- [ ] AC1 — Les 20 dirs `crates/forgia-*` supprimés du disque
- [ ] AC2 — `Cargo.toml` racine : 20 lignes `members` supprimées + 20 lignes `workspace.dependencies` supprimées
- [ ] AC3 — `xtask/no-scaffold-allowlist.toml` : retirer les 20 entries (sinon ratchet bloque déjà sur entries fantômes)
- [ ] AC4 — `cargo check --workspace` PASS (0 erreur)
- [ ] AC5 — `cargo clippy --workspace --no-deps` PASS (0 warning ratchet local — `-W warnings` pas global, voir mémoire 2026-05-23)
- [ ] AC6 — `cargo xtask no-scaffold` exit 0
- [ ] AC7 — `cargo test --workspace --no-run` PASS (vu que forgia-mod-outline disparu, l'erreur `bevy_mod_outline 0.12` connue doit disparaître)
- [ ] AC8 — Cargo.lock régénéré (perte transitive deps anticheat/steam/oxr/scripting-luau notamment)
- [ ] AC9 — 4 commits propres (1 par cluster A/B/C/D) pour faciliter rollback
- [ ] AC10 — PR #2 créée, runtime smoke test (`cargo run -p forgia-game` boot OK)

---

## 4. Anti-patterns à éviter

- ❌ Bulk delete sans vérifier consumers à chaque crate (cf bug story-512 vs forgia-net-* story-468 M5)
- ❌ Oublier d'enlever les entries `[workspace.dependencies]` (membre + dep entry sont 2 sites distincts)
- ❌ Oublier de retirer l'entrée de l'allowlist no-scaffold (ratchet rapportera "stale entry")
- ❌ `git add -u .` sur Windows aspire stowaways (cf mémoire `feedback_git_add_u_stowaway_persists.md`) → toujours `git rm` explicite

---

## 5. Stories follow-up planifiées (post-516)

- **story-517** : FUSION UI (12) + Weapons (5) → 2 meta-crates via pattern macro
- **story-518** : Genome cleanup (5 crates delete/implement)
- **story-519** : Input + Audio IMPLEMENT (12 crates, consumers existent — BMAD Enterprise)
- **story-520** : Player controllers + Misc (8 crates)
- **story-XXX architectural** : split forgia-rpg/lib.rs (1952 LOC monolithique)

---

## 6. Cross-refs

- `docs/audit/scaffolds-audit-2026-05-23.md` — plan source
- `docs/stories/story-512-workspace-purge-vague-1-4.md` — pattern delete
- `docs/stories/story-513-pp-fusion-postprocess.md` — pattern fusion (réservé story-517)
- Mémoires : `feedback_git_add_u_stowaway_persists.md`, `reference_xtask_no_scaffold_ratchet.md`, `feedback_high_incoming_not_god_object.md`
