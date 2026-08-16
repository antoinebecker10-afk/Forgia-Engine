# story-709 — Missions : la boussole de puissance, pas une liste de corvées

**Statut** : DRAFT (2026-08-13)
**Épic** : E3 (surface joueur) · **Scale** : Standard
**Décision source** : [GDD §6 accueil](../design/gdd-forgia-the-spared.md) · [GDD §7 La puissance](../design/gdd-forgia-the-spared.md)

## Le changement de rôle

La page `Missions` promet aujourd'hui *« les défis quotidiens et hebdomadaires »* — un système qui
n'existe pas et qui n'est plus voulu. Elle devient autre chose : **la boussole qui dit au joueur ce
qui le rapproche du gate suivant.**

Faire des Expéditions et des Arènes, monter en puissance, enchaîner les niveaux, atteindre le cap
avec le meilleur stuff — pour ouvrir l'univers d'après. La page rend ce chemin **lisible**, elle ne
le récompense pas.

## 🔑 Elle ne calcule rien — elle lit un capteur qui existe

`forgia2_power.json` décompose déjà la puissance, à 1 Hz :

```json
"power": { "total":2.010, "boons":1.000, "perm":1.396, "mastery":1.200,
           "trempe":1.000, "equip":1.200, "modeled":1.000 },
"wall":  { "lazy":7, "diligent":15 },
"margin":{ "modeled":0.744, "real":0.873 }
```

Chaque composante a déjà son slot — y compris `perm`, qui deviendra la contribution de l'arbre de
talents quand l'Enclume disparaîtra (story-706). **Le travail est une traduction en français, pas un
système.** C'est aussi ce qui rend enfin ce capteur utile au joueur, et pas seulement au dev.

## ⚠️ La règle qui protège l'économie

**Une mission ne donne AUCUNE récompense.** Zéro ressource, zéro Éclat, zéro XP.

Sans cette règle, Missions devient une troisième source de monnaie et perce le trou que
[story-708](story-708-deux-monnaies-etanches.md) vient de boucher — le jalon P2 dit
« aucune récompense croisée ». La récompense d'une mission, c'est **la puissance qu'elle t'a fait
gagner**, et le gate qu'elle ouvre.

## ⚠️ Le piège de la checklist

Une liste de dix objectifs transforme un roguelite en corvée. Le motif qui fonctionne (les Junctions
de Warframe, la Fated List de Hades) affiche **le prochain pas**, pas un arriéré.

**Contrat d'affichage : au plus 3 lignes visibles à la fois, et la première est celle qui bouge le
plus l'aiguille.**

## Critères d'acceptation (falsifiables)

- [ ] La page dit, en français, **où en est la puissance** et **ce qui manque** pour le gate suivant —
      chiffres tirés de `forgia2_power.json`, jamais recalculés
- [ ] **Test** : `power.total` donné ⟹ la ligne affichée est déterministe (fonction pure, testable
      headless, comme `wave_counter`)
- [ ] **Grep : aucune table de récompense n'est associée à une mission.** C'est le test de la règle
      ci-dessus, et il est automatisé
- [ ] Jamais plus de 3 objectifs affichés
- [ ] Si le capteur est absent ou périmé, la page le **dit** — elle n'affiche pas de zéros silencieux
      (règle story-699)
- [ ] La page ne promet plus rien qu'aucun système ne verse (doctrine story-680 cran 1)
- [ ] 0 warning clippy · tests verts

## Dépendance dégradable

E3 (Puissance, gates d'univers) n'existe pas encore. La version v0 se contente du capteur actuel :
« ta puissance vient à 60 % de ton équipement, ton arme est au niveau 3 sur 6 ». Les gates d'univers
s'ajoutent quand E3 arrive — la page n'est pas bloquée par lui.

## Fichiers attendus

- `crates/forgia-menu-hub/src/registry.rs` — `draw_missions` réécrite
- `crates/forgia-menu-hub/src/` — lecture du capteur `power`
- `assets/genomes/roguelite/roguelite_progression.toml` — libellés des objectifs (**data**)

## Dépendances

Aucune bloquante. Gagne en précision après story-706 (la composante `perm` change de sens) et E3.
