# Audit — observabilité de l'avatar (2026-08-16)

**Question posée** : comprend-on exactement ce qui se passe sur le personnage ?
**Réponse courte** : l'instrumentation est riche — c'est la **découverte** et le
**cadrage** qui manquent. On mesure beaucoup d'états, presque aucune intention.

Cet audit ne part pas d'une grille théorique : il part des **quatre défauts qui
ont coûté une session entière** aujourd'hui. Chacun désigne un angle mort réel.

---

## 1. Ce qu'on a — inventaire mesuré

| Source | Contenu | Fraîcheur |
|---|---|---|
| `forgia2_castle_avatar.json` | monté, maillages/visibles, distance caméra, écart au sol, vitesse + **max vue**, lecteurs d'anim actifs, clips attendus/présents **par corps** | vivante |
| `forgia_bone_trace.json` | hiérarchie d'os : nom, profondeur, position monde, rotation locale en degrés | vivante, **25 Ko / 2 s** |
| `forgia_bone_trace_health.json` | désync entre l'AABB du maillage et l'os racine | vivante — **en alerte depuis des jours** |
| `forgia_anim_layer.json` | budgets µs : spring, locomotion, IK | vivante |
| `forgia2_expedition_arme.json` | ancrage, calibrage, échelle, bouche, **écart canon/regard** | vivante |
| `forgia2_expedition_visee.json` | facteur de visée, zoom, FOV, os trouvés/attendus | vivante |
| `forgia2_auto_rig.json`, `forgia2_skeleton_template_registry.json`, `forgia2_equipment.json` | rig auto, gabarits, équipement | vivantes |
| `forgia2_rex_bones*.json` | os du personnage RPG | **périmés (10/08)** |

C'est déjà beaucoup. Le problème n'est pas la quantité.

---

## 2. Les cinq angles morts, chacun payé aujourd'hui

### 2.1 🔴 La découverte cachait un quart des sources — **corrigé**

`forgia_digest.py` ne balayait que `forgia2_*.json`. **Trente capteurs**, écrits
sous le préfixe V1 `forgia_*`, étaient invisibles — dont `bone_trace`,
`anim_layer`, `hitscan`, `combat`, `damage_dir`, `killfeed`.

Le coût n'est pas « une lecture incomplète », c'est **une conclusion fausse** :
j'ai lu l'absence de `forgia2_hitscan.json` comme « aucun tir n'a été résolu »,
alors que `forgia_hitscan.json` existait et était frais. On ne peut pas conclure
d'une absence quand l'outil de découverte ment sur ce qui existe.

Motif élargi aux deux préfixes : **132 capteurs lus au lieu de 102**.

### 2.2 🔴 `bone_trace` itère les MAILLAGES, pas les SQUELETTES

Le chien d'expédition a **8 sous-maillages skinnés** (museau, gueule, blanc de
l'œil, iris, pupille, sac, corps, tissu). Le capteur les compte comme **8
personnages**, plafonne à `max_characters = 8`, et relève pour chacun les
`max_bones = 24` premiers os.

Conséquence : le budget entier part sur **huit copies des mêmes 24 premiers os**
(Hips, Spine, Spine1, Spine2…), et **`RightArm`, `RightForeArm`, `RightHand` ne
sont jamais relevés**. Exactement les os du défaut du jour.

Un plafond qui coupe toujours au même endroit ne mesure pas « un échantillon » :
il mesure **toujours la même chose**, et rend le reste structurellement invisible.

### 2.3 🟠 On mesure des ÉTATS, presque aucune INTENTION

On sait la rotation d'un os. On ne sait pas **si la main est devant le
personnage**. Ce sont deux questions différentes, et seule la seconde est
falsifiable sans lire un rig.

Le seul chiffre d'intention qui existe — `ecart_visee_deg`, l'angle entre le
canon et le regard — a été ajouté **en cours de session**, et il a immédiatement
tranché ce que trois captures d'écran n'avaient pas tranché (71,7°).

Manquent, du même genre : la main est-elle à sa cible · les pieds touchent-ils
le sol · le regard et le buste sont-ils cohérents · l'arme est-elle dans le
champ de la caméra.

### 2.4 🟠 Un capteur qui crie sans lecteur

`bone_trace_health` signale `desync 8/8 — skinning probablement cassé` depuis des
jours. Personne ne l'a lu (cf. 2.1), et **personne n'a vérifié s'il dit vrai** :
comparer le centre d'AABB d'un sous-maillage (un museau) à l'os racine (les
hanches) produit forcément un écart d'un mètre. Il est probablement en **faux
positif permanent** — donc inutilisable même une fois visible.

### 2.5 🟠 Zéro observabilité visuelle, et le monde vivant hors d'atteinte

Tout est en JSON. Or « les bras sont dans le dos » se juge en **une image**, pas
en 25 Ko de quaternions. Et la feature `dev-brp` (Bevy Remote Protocol) est
**déclarée dans le `Cargo.toml` et jamais lancée** — alors que les outils MCP
correspondants sont installés. J'ai passé la session à écrire des capteurs pour
des questions ponctuelles auxquelles une requête BRP répondrait en un appel.

---

## 3. Ce qui existe dehors et vaut le détour

| Outil | Ce qu'il apporte ici | Coût |
|---|---|---|
| **BRP + `bevy_brp_mcp`** | Interroger le monde ECS **en direct** : transform d'un os, composants d'une entité, ressources. Déjà installé côté MCP, feature déjà déclarée | **Lancer avec `--features dev-brp`.** Zéro code |
| **`bevy_mod_skinned_aabb` 0.4** | Calcule les AABB des maillages skinnés (on a un **contournement** : `sys_disable_frustum_culling_on_avatar`) **et** affiche les AABB par joint (touches J/M). Version 0.4 = Bevy 0.18, exactement la nôtre | +4 % CPU sur les skinnés. Bevy 0.19 l'intègre nativement → pont assumé |
| **`bevy_gizmos`** (intégré) | Aucune crate de squelette dédiée n'existe. Dessiner les os = ~40 lignes maison | nul |
| `bevy-inspector-egui`, `bevy_debugger_mcp` | Redondants avec BRP+MCP pour notre usage | — |

---

## 4. Plan, par rapport valeur/effort

| # | Chantier | État | Ce que ça débloque |
|---|---|---|---|
| 0 | Découverte élargie aux deux préfixes | ✅ | **132 capteurs lus au lieu de 102** |
| 1 | `bone_trace` squelette-centrique + os surveillés par nom | ✅ | Les bras sont enfin dans le relevé |
| 2 | BRP dans la commande de dev quotidienne | ✅ | Le monde vivant, sans écrire de capteur |
| 3 | Gizmo de squelette (F3) + `bevy_mod_skinned_aabb` | ✅ | Une pose se juge en une image |
| 4 | Mesures d'intention : main↔cible, buste↔regard | ✅ | Des défauts falsifiables sans lire un rig |
| 5 | Honnêteté de `bone_trace_health` | ✅ | Il compare enfin des choses comparables |

### Détail de ce qui a été livré

**0 — Découverte.** `forgia_digest.py` balaie les deux préfixes. Le fichier
`forgia_bone_trace_health.json`, en alerte depuis des jours, est apparu au
premier appel.

**1 — `bone_trace` refait.** Regroupement par **squelette** (clé = premier
joint : deux maillages du même personnage partagent la même liste). Les os
**surveillés** sont relevés par nom, **hors plafond** ; le reste remplit ce qui
reste ; et `os_ecartes` publie combien ont été laissés de côté — un relevé
tronqué qui ne dit pas qu'il tronque se lit comme un relevé complet.

Au passage : `config/genomes/debug_anim.toml` **n'était lu par personne**. Son
en-tête se disait « hot-reloadable » depuis story-454, aucun code ne l'ouvrait,
et les valeurs qui tournaient étaient celles écrites en dur. Il est lu, et un
test échoue s'il redevient décoratif.

**2 — BRP.** L'alias `cargo forgia-brp` existait **et n'a jamais servi** : chaque
récap de test disait `cargo run -p forgia`. Un outil qui a sa propre commande est
un outil qu'on oublie. `dev-brp` est donc entré dans `cargo forgia-dev`, la
commande de tous les jours, à côté de Tracy. Le build joueur n'a ni l'un ni
l'autre.

**3 — Le visuel.** `squelette_gizmo` (F3) dessine un segment par os — la
silhouette — et un **repère d'axes locaux** sur les os surveillés. C'est ce
repère qui répond à « dans quel sens tourne cet os », la question sur laquelle
douze angles d'Euler devinés se sont cassé les dents. `bevy_mod_skinned_aabb`
0.4.1 calcule les AABB des maillages skinnés (J/M pour les voir).

**4 — Intention.** `ecart_main_droite_m` / `_gauche_m` (ce qui reste entre la
main et son point demandé), `ecart_buste_deg`, et l'alerte `MAIN_HORS_CIBLE`.
La cible est aussi **publiée pour être dessinée** à côté de la main : la question
devient une évidence au lieu d'un calcul.

**5 — Santé honnête.** L'écart se mesure entre le centre d'AABB d'un maillage et
le barycentre de **ses propres joints**. L'ancien comparait le museau du chien à
ses hanches — un mètre d'écart légitime, donc une alerte permanente. Et le cas
« aucun squelette » rend `info: AVEUGLE` au lieu de supprimer le fichier comme si
tout allait bien.

### Reste ouvert, délibérément

`sys_disable_frustum_culling_on_avatar` (dans `forgia-mode-roguelite/avatar.rs`)
est un **contournement** du défaut que `bevy_mod_skinned_aabb` corrige à la
source. Il peut tomber — mais le fichier est en cours d'édition par un autre
terminal, et le retirer demande une validation en jeu. À faire quand les deux
conditions seront réunies.

---

## 5. Le principe qui manquait

Les capteurs de ce projet répondent très bien à **« quel est l'état ? »**. Les
quatre défauts du jour demandaient tous **« est-ce que le résultat voulu est
atteint ? »** — une question que presque aucun ne pose.

> Un capteur d'état vieillit avec le code. Un capteur d'intention vieillit avec
> le jeu. C'est le second qui trouve les bugs qu'on n'a pas prévus.

---

*Cross-refs : `observability-required.md` · `log-digest.md` ·
`map-design-patterns.md` §13-14 (« zéro mesuré n'est pas vert ») ·
[story-717](../stories/story-717-combat-troisieme-personne-expedition.md)*

*Sources externes : [bevy_mod_skinned_aabb](https://github.com/greeble-dev/bevy_mod_skinned_aabb) ·
[bevy_brp_mcp](https://lib.rs/crates/bevy_brp_mcp) ·
[Bevy Remote Protocol](https://docs.rs/bevy_remote) ·
[bevy_gizmos](https://docs.rs/bevy_gizmos)*
