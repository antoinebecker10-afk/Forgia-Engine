# Story-604 — Watchdog freeze externe (observabilité robustesse ship)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `server.rs`, symbole `AtomicU64`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
> **Niveau BMAD** : Standard (thread OS + 2 sensors + champ snapshot)
> **Valeur** : **HIGH** — stabilité ship Roguelite
> **Origine** : feature V1 non migrée vers V2. Spec de référence = `tools/forgia-mcp/src/server.rs` (`read_watchdog_heartbeat` / `read_watchdog_alert`) + le watchdog du jeu V1 (`forgia_watchdog_heartbeat.json` / `forgia_watchdog_alert.json`).

## Problème
V2 n'a qu'un compteur de frames **sur le thread principal**. S'il gèle (deadlock GPU/asset, boucle infinie), le compteur s'arrête → **impossible de s'auto-alerter**. Un freeze chez un playtester = **zéro trace**. Le champ `seconds_in_emergency` est déjà **lu** par `crates/forgia-debug/src/snapshot.rs` mais **jamais émis** (orphelin).

## À construire
- **Thread OS séparé** (`std::thread`) qui lit un heartbeat `AtomicU64` (timestamp ms) bumpé chaque frame par le main loop.
- Heartbeat continu → `forgia2_watchdog_heartbeat.json` { ts_unix_ms, last_main_heartbeat_ms, gap_ms, main_thread_alive } à 1 Hz.
- Si `now - last_heartbeat > 5s` → écrire `forgia2_watchdog_alert.json` { alert_at_ms, gap_ms, threshold_ms, note } **depuis le thread externe** (vit même si le main gèle).
- Renseigner le champ orphelin `seconds_in_emergency` consommé par `snapshot.rs`.
- Crate : `forgia-observability` (framework existant).

## Acceptance
- [ ] Le thread écrit l'alerte même quand le main-thread est artificiellement gelé (test : `std::thread::sleep(6s)` dans un system).
- [ ] `forgia2_watchdog_alert.json` absent en run normal, présent après un gel > 5s.
- [ ] `snapshot.rs::seconds_in_emergency` reflète la durée de gel.
- [ ] 0 alloc hot path (heartbeat = `store` atomique).
