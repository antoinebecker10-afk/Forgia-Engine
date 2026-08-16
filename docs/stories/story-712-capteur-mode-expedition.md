# Story 712 — Un capteur pour le mode Expédition

**Statut** : DRAFT
**Niveau BMAD** : Quick (≤ 3 fichiers)
**Bloque** : 713, 714, 715, 716 — on ne règle pas un combat qu'on ne peut pas lire

---

## Pourquoi elle passe en premier

Le 2026-08-14, un défaut visible — **deux avatars superposés, dont un bras
écartés** — a survécu à trois vérifications de ma part et n'a été trouvé que
par une capture d'écran de l'utilisateur. Cause : la crate `forgia-mode-expedition`
**n'écrit aucun capteur**, et `forgia2_run.log` datait de 16 h 44 pendant que la
partie tournait à 21 h 30.

`observability-required.md` est bloquante et dit exactement ça : *« quand l'user
dit "regarde", l'IA doit pouvoir diagnostiquer la feature en lisant sa sortie.
Si l'IA ne voit rien, la feature est incomplète. »*

Mettre trois campements et une IA de combat dans un mode aveugle, c'est se
garantir la même séance de diagnostic à l'œil, mais sur un sujet dix fois plus
mouvant.

## Ce qu'il faut voir

`forgia2_expedition.json`, écrit à ~1 Hz :

```json
{
  "avatars": 1,                    // 2 = le bug de doublon est de retour
  "corps_anime": true,             // false = bras écartés
  "sockets_equipes": 3,            // 0 = aucun feu accroché
  "braseros": {"poses": 16, "allumes": 4},
  "progression_chemin": 0.42,
  "soleil_elevation_deg": 11.3,
  "campements": [{"id": "camp_1", "vivants": 3, "etat": "endormi"}],
  "faune_vivante": 24
}
```

## Critères d'acceptation

- [ ] `forgia2_expedition.json` écrit périodiquement pendant que le mode tourne
- [ ] La période est un gène, pas un littéral (`no-hardcode.md`)
- [ ] **Une alerte santé** « AVATAR DOUBLE » si `avatars > 1`, avec son next-step
- [ ] **Une alerte santé** « CORPS FIGÉ » si `corps_anime == false` après N passes
- [ ] Le capteur distingue **0 mesuré** de **rien à mesurer** : hors Expédition il
      ne doit pas écrire `0` partout, il ne doit **rien** écrire
      (`map-design-patterns.md` §13 — « zéro mesuré n'est pas vert, c'est aveugle »)
- [ ] `python tools/ai/forgia_digest.py sensors` le voit

## Fichiers

- `crates/forgia-mode-expedition/src/capteur.rs` (nouveau)
- `crates/forgia-mode-expedition/src/plugin.rs` (câblage, 1 ligne)
- `assets/genomes/expedition_vfx.toml` ou un `expedition_debug.toml` (période)

## Risque

**Bas.** Lecture seule sur l'état existant, aucun chemin chaud touché.

## Cross-refs

- `.claude/rules/observability-required.md`
- `[[feedback_verifier_le_rendu_reel_pas_le_html]]` — un contrôle hors contexte
  est aveugle, pas rassurant
