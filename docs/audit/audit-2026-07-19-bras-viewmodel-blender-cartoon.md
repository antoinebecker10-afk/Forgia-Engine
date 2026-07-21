# Audit — Bras viewmodel AA via Blender pour la DA cartoon (2026-07-19)

> **Question** : peut-on brancher Claude sur Blender pour créer des bras first-person
> de qualité AA, style « cartoon rigolo », remplaçant les bras procéduraux actuels ?
> **Méthode** : audit local (code + assets + Blender installé) + 3 recherches web
> parallèles (Blender↔IA, pipeline bras FPS cartoon, génération 3D par IA).

---

## TL;DR — Verdict

| Question | Réponse |
|---|---|
| Se brancher sur Blender ? | **OUI — prouvé aujourd'hui** (Blender 5.0 installé, piloté en headless pour inspecter un GLB) |
| L'IA sculpte des bras cartoon AA from scratch ? | **NON** — le sculpt organique est le point faible documenté de tous les ponts Blender-IA (et inaccessible en headless) |
| La génération 3D IA (Meshy/Tripo/Rodin) pour des bras ? | **NON** — mains/doigts = pire failure mode de la catégorie, auto-rig impossible sur bras seuls |
| Alors quoi ? | **Base CC0 riggée+animée existante → « cartoonisation » scriptée dans Blender → export GLB → intégration dans forgia-viewmodel existant.** Budget ~2-3 jours, 0 € |

Le partage des rôles que toutes les sources dessinent : **l'agent IA excelle en pipeline
(retouche paramétrique, proportions, retexture, rig par script, export) — pas en
artistique organique.** La qualité AA vient de l'asset de départ, pas du sculpt IA.

---

## 1. Branchement Blender — 3 voies possibles

| Voie | État | Usage |
|---|---|---|
| **Headless scripté** — `blender.exe --background --python script.py` | ✅ **Opérationnel aujourd'hui** (Blender 5.0 dans `C:\Program Files\Blender Foundation\Blender 5.0`) | Pipeline reproductible et versionnable : inspection, retouche paramétrique, retexture, export GLB. **Voie recommandée.** |
| **blender-mcp** (ahujasid, ~24,5k stars) | Installable (`claude mcp add blender -- uvx blender-mcp` + addon) | Sessions interactives avec **screenshots du viewport** (exige Blender GUI ouvert). ⚠️ Télémétrie ON par défaut → `DISABLE_TELEMETRY=true`. |
| **MCP officiel Blender Foundation** (Blender Lab) | Nécessite Blender **5.1+** (addon via Get Extensions) — on a 5.0 | Alternative « petite et maintenable » ; upgrade 5.2 LTS (sortie 14/07/2026) si besoin. |

**Preuve locale** : inspection headless de `assets/models/fps_arms.glb` réussie
(script Python → objets, armature 85 bones, 31 371 tris, 1 animation, textures 1024²).

### Ce qui marche bien / mal en Blender scripté (sources convergentes)

- ✅ **Fiable** : data-API mesh (`from_pydata`, `bmesh`), modifiers, armatures + poids posés
  par script, shape keys, retexture, **export glTF/GLB skinné+animé** (usage CI standard).
- ❌ **Hors de portée** : sculpt organique (opérateurs exigent un viewport — bug T73321),
  topologie propre « pensée edge-loops », skinning fin des doigts. Une main est le cas
  organique le plus difficile.
- Règle pratique : **data-API > `bpy.ops`** partout où c'est possible en headless.

---

## 2. État local

- **Bras actuels** : [arms.rs](../../crates/forgia-viewmodel/src/arms.rs) (575 LOC) —
  poings procéduraux (capsules/sphères), texture peau bruit procédural, 3 styles
  cosmétiques (Peau/Gantelet/Cyber), **placement auto par-arme** dérivé du transform
  réel de l'arme (genome). Le fichier assume son plafond (ligne 14) :
  *« procédural = stylisé. Réalisme poussé = mesh de mains riggé (asset) »*.
- **Asset dormant** : `assets/models/fps_arms.glb` (10,5 Mo, 14/05/2026) = mains FPS
  **réalistes** Sketchfab + fusil SCAR (85 bones, 31k tris, 1 anim). Référencé **nulle
  part** dans le code. Style militaire réaliste → hors DA cartoon ; licence Sketchfab
  non tracée → ne pas shipper sans vérif. Utile comme référence de structure de rig.
- **Plomberie déjà prouvée** : armes-créatures GLB dans le viewmodel (auto-calibration
  AABB `NeedsAutoScale`), animation squelettique GLB maîtrisée côté ennemis
  (`AnimationPlayer`/`AnimationGraph`, story-636). Charger un GLB de bras riggés =
  technologie connue du codebase.
- **Nuance DA existante** : le viewmodel n'est PAS toon-shadé (caméra séparée,
  commentaire arms.rs:154-156) — le détachement visuel se fera par aplats + valeurs,
  comme Gunfire Reborn (pas d'outline viewmodel non plus chez eux → notre outline OFF
  n'est pas bloquant).

---

## 3. Génération 3D par IA — verdict mi-2026

| Objet | Verdict |
|---|---|
| Props/décor cartoon (tonneaux, cristaux…) | ✅ Mûr — Rodin (quads propres) ou TRELLIS.2 local (MIT, 16 GB VRAM). Vrai sweet spot 2026. |
| Ennemis/NPC de masse | 🟡 Base correcte + auto-rig Tripo/Meshy (humanoïdes complets vus à distance) |
| **Bras FPS riggés** | ❌ Inférieur à un asset pack : (1) mains/doigts = pire failure mode documenté *de tous* les générateurs ; (2) auto-rig exige un humanoïde complet T-pose — aucun outil ne rigge des bras seuls ; (3) les anims viewmodel (reload/inspect) n'existent dans aucune bibliothèque IA |

**Pièges licence** : Hunyuan3D = Territory **exclut l'UE** → juridiquement inutilisable
pour nous. Meshy/Tripo free = CC-BY (attribution) ; commercial plein = plan payant.
Luma Genie = abandonné (01/2026). TRELLIS (Microsoft) = MIT, seul choix local sérieux.

---

## 4. DA — ce qui fait qu'un bras cartoon « lit bien »

Références : TF2 (papier Valve NPAR07 + GDC08 « Stylization With A Purpose »),
Roboquest (artbook ~8 € sur Steam — meilleur ratio prix/valeur de cet audit),
Gunfire Reborn (viewmodels volontairement très gros à l'écran).

1. **Proportions exagérées** : mains ×1,5–2, avant-bras évasés (à FOV viewmodel serré,
   une main réaliste paraît chétive).
2. **4 doigts** (3 + pouce) : convention cartoon, gestes plus lisibles, rig plus simple.
3. **Gants/mitaines** : pas de peau à réussir + silhouette renforcée + couleur signature.
   → **Gants de forgeron** = cohérent avec l'identité forge/enclume de Forgia.
4. **Aplats + toon ramp** : texture palette 1 px (style KayKit, déjà nos ennemis),
   metallic 0 / roughness 1, 2-3 valeurs max.
5. Lisibilité en **silhouette pure** (leçon TF2) ; rim/contraste de valeur > outline.

---

## 5. Bases candidates (asset de départ)

| Base | Licence | Contenu | Verdict |
|---|---|---|---|
| **PSX First Person Arms — Drillimpact (itch.io)** | **CC0** | FBX/**GLB/Blend**, riggé, **~17 anims**, variante gants | ⭐ Meilleure base : tout inclus, licence parfaite, à cartooniser |
| cartoon FPS Arms — DJMaesen (Sketchfab) | CC-BY (crédit) | 1,3k tris, riggé, **sans anims** | Seul « cartoon » natif ; combinable avec les anims Cransh faites pour ce mesh |
| Découpe d'un perso KayKit (bras isolés, squelette gardé) | CC0-friendly | 161 anims KayKit dispo | Cohérence DA gratuite avec nos ennemis ; travail de découpe Blender |
| Synty POLYGON (source FBX) | EULA engine-agnostic (Bevy OK) | 20-60 $/pack | Si besoin d'un style flat-color plus riche |
| Freelance (fallback) | — | 500–1 000 $ réaliste (bras + rig + 6 anims) | V2 polish, avec le proto comme brief |

KayKit/Quaternius n'ont **pas** de pack « FPS arms » dédié (persos full-body only).

---

## 6. Pipeline retenu (recommandation)

**Option A — base CC0 cartoonisée, ~2-3 jours, 0 €** :

1. **Import + audit** de la base (Drillimpact) en Blender headless (script versionné).
2. **Cartoonisation scriptée** : scale mains ×1,7, avant-bras évasés, fusion 5→4 doigts
   si nécessaire, retexture palette d'aplats 32×32, gants forgeron couleur signature
   (mapper sur les 3 styles `ArmCosmetics` existants : Peau/Gantelet/Cyber).
3. **Export GLB** — pièges connus : `Ctrl+A` transforms appliqués (mesh ET armature),
   1 unité = 1 m sans pré-rotation, poids normalisés (max 4 influences/vertex),
   1 action nommée par clip (`idle`/`fire`/`reload` → clés `named_animations` Bevy),
   « Deform bones only », matériaux base color plate (le toon éventuel reste côté moteur).
4. **Intégration Bevy** en 2 incréments :
   - **Inc.1** : swap des primitives de `spawn_hand` par le mesh GLB en pose grip
     statique — on **garde** le placement auto par-arme, le sway/bob procédural
     (validé FixedUpdate) et `update_arms_visibility`. Gain visuel immédiat.
   - **Inc.2** : anims bakées (draw/reload/inspect) via `AnimationPlayer` (pattern
     story-636), le procédural gardant idle/bob — hybride standard du genre.
5. **Pièges maison** : matériaux GLB partagés → clone/dédup AssetId avant tint
   `ArmCosmetics` ; `NotShadowCaster` sur les bras ; lumières `RenderLayers` si caméra
   viewmodel dédiée (issues Bevy #18000/#20878 → tester les ombres dès l'intégration) ;
   auto-calibration AABB plutôt que scale hardcodé (règle no-hardcode).

**Si le test in-game ne « lit » pas assez cartoon** → escalade freelance (~500-1 000 $)
avec le prototype comme brief visuel.

---

## 7. Sources clés

- Blender↔IA : [blender-mcp](https://github.com/ahujasid/blender-mcp) ·
  [MCP officiel Blender](https://www.blender.org/lab/mcp-server/) ·
  [bpy pip](https://pypi.org/project/bpy/) ·
  [sculpt headless T73321](https://developer.blender.org/T73321) ·
  [retour d'expérience MindStudio](https://www.mindstudio.ai/blog/claude-blender-mcp-real-world-performance)
- Pipeline viewmodel : [exemple Bevy first_person_view_model](https://bevy.org/examples/camera/first-person-view-model/) ·
  [export Blender→glTF (ezEngine)](https://ezengine.net/pages/docs/animation/skeletal-animation/blender-export.html) ·
  [glTF skeletal (lisyarus)](https://lisyarus.github.io/blog/posts/gltf-animation.html)
- DA : [Valve NPAR07 TF2](https://steamcdn-a.akamaihd.net/apps/valve/2007/NPAR07_IllustrativeRenderingInTeamFortress2.pdf) ·
  [GDC08 Stylization With A Purpose](https://cdn.akamai.steamstatic.com/apps/valve/2008/GDC2008_StylizationWithAPurpose_TF2.pdf) ·
  [artbook Roboquest](https://store.steampowered.com/app/2729110/Roboquest__Digital_Art_Book/)
- Assets : [Drillimpact PSX arms (CC0)](https://drillimpact.itch.io/psx-first-person-arms-free) ·
  [DJMaesen cartoon arms (CC-BY)](https://sketchfab.com/3d-models/cartoon-fps-arms-25d06c227fa3419b92fe65f39887b0b8) ·
  [KayKit anims](https://kaylousberg.itch.io/kaykit-character-animations) ·
  [licences Synty](https://syntystore.com/pages/licences-overview)
- Gen 3D IA : [test 9 outils (Indie Hackers)](https://www.indiehackers.com/post/best-ai-3d-model-generator-in-2026-i-tested-9-of-the-best-and-here-is-what-i-found-70ecab1a0a) ·
  [auto-rigging showdown (StraySpark)](https://www.strayspark.studio/blog/ai-auto-rigging-showdown-2026-tripo-meshy-cascadeur-mixamo) ·
  [TRELLIS MIT](https://github.com/microsoft/TRELLIS) ·
  [Hunyuan3D licence hors-UE](https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1/blob/main/LICENSE)

---

*Audit réalisé le 2026-07-19 (session Blender/bras viewmodel). Prochaine étape
proposée : story « bras viewmodel GLB cartoon » (BMAD Standard — ≥2 implémentations :
pipeline Blender + intégration forgia-viewmodel).*
