# story-696 — Le hitstop ne se déclenche jamais, et son capteur s'est tu

**Statut** : DRAFT
**Créée** : 2026-08-12
**Niveau BMAD** : Quick (diagnostic d'abord — la correction dépend de ce qu'on trouve)
**Origine** : run de validation du 2026-08-12, lecture des capteurs de game feel.
**Bloque** : [story-648](story-648-paliers-hitstop.md) (paliers de hitstop), qui ne
peut pas être validée tant que l'effet ne part pas.

---

## Symptôme, mesuré

`forgia2_gamefeel.json`, après une run complète avec **51 kills** et **419 hits** :

```json
"hitstop_counts": { "hit": 0, "crit": 0, "kill": 0, "multikill": 0 },
"last_tier": "none",
"last_duration_ms": 0
```

**Zéro hitstop sur toute la run.** Les paliers sont pourtant chargés et lisibles
dans le même capteur (`crit_mult 1.50`, `kill_mult 2.50`, `multikill_mult 4.00`),
donc la configuration arrive — c'est le déclenchement qui manque.

## Le piège : deux hypothèses, et le capteur ne les départage pas

`timestamp_secs: 218.4` alors que les capteurs voisins sont à **419,5**. Le fichier
a cessé d'être écrit à mi-run. Donc :

- **(a)** le hitstop ne se déclenche jamais → compteurs à 0, c'est un vrai défaut ;
- **(b)** le producteur a cessé de tourner à t=218 → les compteurs sont un
  instantané périmé, et l'effet marche peut-être depuis.

**Les deux lectures sont compatibles avec la donnée.** C'est exactement pourquoi le
chien de garde a été étendu le même jour (`sensor_health` surveille désormais les
128 capteurs, pas 13) : cet arrêt-là n'avait déclenché **aucune** alerte.

## Marche à suivre

1. **Relancer une run après le fix du chien de garde.** Si `sensor_health` signale
   `forgia2_gamefeel.json` dans `stalled_paths`, l'hypothèse (b) est confirmée et
   le vrai bug est dans le système qui écrit le capteur, pas dans le hitstop.
2. Si le capteur suit la run et que les compteurs restent à 0 → hypothèse (a),
   chercher le producteur des `hitstop_counts` et pourquoi il n'incrémente pas.
3. **Ne pas toucher aux multiplicateurs** (`crit_mult`, `kill_mult`,
   `multikill_mult`) : rien ne dit qu'ils sont en cause, et ils sont chargés
   correctement. Cf `no-speculative-fix.md`.

## Critères d'acceptation

- [ ] L'hypothèse (a) ou (b) est **tranchée par une mesure**, pas supposée
- [ ] `hitstop_counts.hit` > 0 après une run de combat
- [ ] Les paliers se distinguent : `kill` déclenche une durée > `hit`
- [ ] `forgia2_gamefeel.json` suit la run de bout en bout (plus d'arrêt à mi-course)
- [ ] story-648 peut alors être validée ou infirmée sur pièces

## Cross-refs

- `crates/forgia-observability/src/sensor_health_sensor.rs` — le chien de garde étendu
- `.claude/rules/observability-required.md` · `map-design-patterns.md` §13
