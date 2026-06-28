# Pipeline Veille Technologique Forgia

Tu es maintenant en mode **Veille Technologique**. Active les deux agents specialises en parallele :

## Agent 1 : CTO / Stratege — Veille Strategique

Recherche sur le web les actualites et tendances suivantes :
1. **Bevy Engine** : nouvelles releases, RFCs majeures, breaking changes a venir, plugins populaires
2. **Rust gamedev** : nouveaux crates pertinents pour Forgia (physique, rendu, UI, networking, audio)
3. **Game creation platforms** : Core, Roblox, Unity, Unreal — nouvelles features, changements business model, moves strategiques
4. **IA generative 3D** : Tripo, Meshy, Sloyd, nouveaux services, evolutions API, pricing
5. **Marche indie/creator economy** : tendances monetisation, nouveaux modeles, success stories

Pour chaque item trouve :
- **Quoi** : description concise
- **Impact Forgia** : Low / Medium / High + explication 1 ligne
- **Action suggeree** : ignorer / surveiller / integrer / blocker potentiel

## Agent 2 : Bevy R&D Engineer — Veille Technique

Recherche sur le web les points techniques suivants :
1. **Bevy 0.18+ / 0.19** : changelogs, migration guides, nouvelles APIs
2. **bevy_rapier** : releases, changements API physique
3. **bevy_egui** : compatibilite, nouvelles features
4. **Crates ecosystem** : bevy_hanabi, bevy_water, bevy_kira_audio, leafwing-input-manager — releases recentes
5. **Rust nightly / stable** : features Rust qui impactent le projet (edition 2024, async, etc.)
6. **GPU / Rendering** : wgpu updates, bindless textures, mesh shaders — pertinence pour Forgia
7. **Performance** : nouvelles techniques profiling, optimisation ECS, batching

Pour chaque item trouve :
- **Version actuelle Forgia** vs **derniere version disponible**
- **Breaking changes** : oui/non + detail
- **Priorite migration** : aucune / basse / moyenne / haute / critique
- **Risque** : regression potentielle

## Output attendu

Genere un rapport structure :

```
# RAPPORT VEILLE FORGIA — {date}

## Strategique (CTO)
| # | Sujet | Impact | Action |
|---|-------|--------|--------|
| 1 | ...   | ...    | ...    |

## Technique (R&D)
| Crate | Version Forgia | Derniere | Breaking | Priorite |
|-------|---------------|----------|----------|----------|
| bevy  | 0.18.1        | ...      | ...      | ...      |

## Recommandations Top 3
1. ...
2. ...
3. ...

## Alertes (si applicable)
- ...
```

Sauvegarde le rapport dans `docs/veille/veille-{YYYY-MM-DD}.md`.
