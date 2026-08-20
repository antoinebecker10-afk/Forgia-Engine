# Prompt de reprise — 2026-08-18 (Arène Test Spared)

> À copier-coller tel quel dans un autre terminal Claude Code.

---

Workspace : `C:\Users\Antoi\Desktop\Forgia Rewrite`

## CONTEXTE

Session du 2026-08-18 terminée. **Rien n'est commité** (187+ fichiers modifiés)
et **aucun `forgia.exe` n'est compilé**. Un autre terminal travaille sur
`forgia-mode-expedition` (`cape.rs`, `posture.rs`, `visee.rs`, `arme_main.rs`) —
ne pas y toucher sans vérifier les mtimes.

Ce qui a été livré et vérifié (`arch-drift` · `no-scaffold` · `check-orphans` ·
`capteur-gate` · `strates` **OK**, clippy `-D warnings` **0**, **216 tests**) :

- `forgia_core::constat` — le type qui rend impossibles les 3 défauts de capteur
- 2 cliquets : `cargo run -p xtask -- capteur-gate` et `-- strates`
- Le jeu écrit son propre `forgia2_run.log` (`LogPlugin.custom_layer`)
- **Le crash de l'Expédition est résolu** : `assets/shaders/expedition_vent.wgsl`
- Maquette Blender `tools/blender/maquettes/arene_test_spared.blend` (549 objets)

## MEMORIES À CHARGER EN PREMIER

```
session_2026_08_18_constat_cliquets_maquette
reference_constat_et_cliquets_capteurs
reference_shader_sommet_bevy_contrat_forward_io
feedback_un_bisect_qui_deplace_le_symptome
reference_maquette_blender_arene_test_spared
feedback_un_instrument_qui_vise_a_cote_fabrique_un_defaut
```

## REPRENDS, DANS CET ORDRE

### ▼ TIER 1 — ce qui rend le reste inutile s'il manque (~45 min, risque bas)

**A. Brancher les 2 cliquets en CI.** Ils existent, ils mordent (testés), et ils
ne tournent nulle part. Un cliquet non branché est une intention.
- Source : `.github/workflows/` (chercher le job `ratchets`)
- Ajouter : `cargo run -p xtask -- capteur-gate` et `-- strates`
- Effort ~30 min

**B. `sensor-audit` est ROUGE** — 2 capteurs absents du registre :
`forgia2_expedition_cape.json` (`cape.rs:233`) et
`forgia2_expedition_posture.json` (`posture.rs:177,240`). Ils viennent de
l'autre terminal → **lui demander** plutôt que documenter à sa place.

### ▼ TIER 2 — la dette déclarée, qui ne peut que baisser (long, risque bas)

**C. 38 capteurs sans verdict** → migrer vers `constat`, un par un.
Chaque migration permet d'abaisser `sans_verdict` dans `xtask/capteur-dette.toml`.
Modèle : `crates/forgia-anim-debug/src/anim_sensor.rs`.

**D. `forgia-observability` dépend de 9 crates de gameplay.** Le capteur d'un
mode doit vivre **dans la crate de ce mode** et n'utiliser que `constat` —
comme le fait déjà `forgia-mode-expedition/src/capteur.rs`. Chaque déménagement
retire une ligne de `xtask/strates.toml`.

### ▼ TIER 3 — dette pointée, non traitée (risque moyen)

**E. `eau.rs:54`** code en dur le chemin du modèle alors que le manifeste
déclare `eau.objet_glb` : champ mort, hardcode. C'est ce qui a rendu un de mes
bisects inopérant.

**F. `bevy_water` 0.18.1** porte le MÊME défaut que le shader de vent
(`water_vertex.wgsl` sous l'ancien `#ifdef VERTEX_UVS`). Substituable sans fork
via les poignées `load_internal_asset!`. **Rien ne prouve qu'il casse quelque
chose aujourd'hui** — ne pas corriger sans symptôme.

**G. Le `.blend` de la maquette (10,9 Mo) n'est ni ignoré ni commité.** Décision
d'Antoine attendue : ignorer le `.blend` et versionner le `.png` (la maquette se
rejoue par script, graines fixes), tout ignorer, ou tout versionner.

## VALIDATION ENTRE CHAQUE TÂCHE

```bash
cargo run -p xtask -- capteur-gate && cargo run -p xtask -- strates
cargo run -p xtask -- arch-drift && cargo run -p xtask -- sensor-audit
"$(rustup which cargo)" clippy -p <crate> --all-targets -- -D warnings
```

⚠️ Ne PAS utiliser `rtk cargo clippy` : RTK masque les lints
(cf. `reference_rtk_wraps_cargo_hides_clippy_lints`).

## POUR TESTER EN JEU

```bash
cargo forgia-dev          # Tracy + BRP — LA commande de dev
```

Le log s'écrit maintenant **tout seul** dans `forgia2_run.log`, quel que soit le
mode de lancement, avec rotation vers `.previous`. Le lire par
`python tools/ai/forgia_digest.py all`, jamais en brut — il crie désormais s'il
est périmé par rapport aux capteurs.

## POUR REPRENDRE LA MAQUETTE

Ouvrir `tools/blender/maquettes/arene_test_spared.blend` (collection racine
`ARENE_TEST_SPARED`, sous-collections `ATS_*`). Ce qui reste à pousser, par
ordre d'écart avec la référence : plus de bois (un deck à deux étages), des
briques individuelles lisibles sur les faces de mur, les engins de siège au
premier plan.

🚨 Lire `reference_maquette_blender_arene_test_spared` **avant** de toucher aux
transformations : `o.location` ment quand la transformation a été appliquée, et
`o.scale` sur un objet d'origine (0,0,0) le **téléporte**.

**GO par la tâche A** — sans elle, tout le reste de la session est décoratif.
