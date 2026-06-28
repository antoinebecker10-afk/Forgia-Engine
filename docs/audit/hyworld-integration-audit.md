# HY-World 2.0 — Audit d'intégration Forgia

> Rédigé le 2026-04-23. Sources : GitHub Tencent-Hunyuan/HY-World-2.0, crates.io, docs.rs.
> Validation requise avant passage à l'étape prototype (step 5).

---

## Étape 1 — Faisabilité technique

### 1.1 Identité du projet

| Champ | Valeur |
|---|---|
| Repo | `Tencent-Hunyuan/HY-World-2.0` |
| Créé | 2026-04-10 (13 jours au moment de l'audit) |
| Dernière MAJ | 2026-04-22 |
| Stars / Forks | 1 583 / 118 |
| Langage | Python |
| Paper | arXiv:2604.14268 |
| Page officielle | https://3d-models.hunyuan.tencent.com/world/ |

### 1.2 Pipeline et état de publication

HY-World 2.0 comporte deux modes :

**Mode Reconstruction** (multi-vues ou vidéo → 3D)
- Modèle : WorldMirror 2.0 (~1,2B paramètres)
- Statut : **DISPONIBLE** — code d'inférence + poids HuggingFace publiés

**Mode Génération** (texte / image → monde 3D)
- 4 étapes : HY-Pano 2.0 → WorldNav → WorldStereo 2.0 → WorldMirror 2.0
- Statut : **PARTIEL** — seul WorldMirror 2.0 est disponible. HY-Pano 2.0, WorldNav et WorldStereo 2.0 sont "coming soon"

> **Implication directe** : on ne peut pas encore piloter la génération depuis texte/image. Seule la reconstruction depuis photos/vidéo est opérationnelle.

### 1.3 Formats d'export

#### Structure de sortie

```
inference_output/<case_name>/<timestamp>/
  gaussians.ply          # 3D Gaussian Splatting (format standard)
  points.ply             # Point cloud RGB (max 2M points par défaut)
  camera_params.json     # Caméras (extrinsics + intrinsics)
  depth/
    image_0001.npy       # float32 [H, W] — depth Z en coordonnées caméra
    image_0001.png       # visualisation
  normal/
    image_0001.png       # normales RGB (coordonnées caméra)
  sparse/                # COLMAP (optionnel, pycolmap 3.10.0)
  rendered/              # MP4 interpolé (optionnel)
```

#### Format `gaussians.ply`

PLY binaire little-endian, **59 propriétés float par vertex** :

| Groupe | Propriétés | Nb | Notes |
|---|---|---|---|
| Position | `x`, `y`, `z` | 3 | Centre du splat |
| Normales | `nx`, `ny`, `nz` | 3 | Toujours 0 en 3DGS standard |
| SH DC | `f_dc_0`, `f_dc_1`, `f_dc_2` | 3 | Couleur base (ordre 0) |
| SH reste | `f_rest_0` … `f_rest_44` | 45 | Couleur view-dependent (bands 1-3) |
| Opacité | `opacity` | 1 | Pre-sigmoid → `sigmoid(x)` → [0,1] |
| Échelle | `scale_0`, `scale_1`, `scale_2` | 3 | Log-scale → `exp(x)` |
| Rotation | `rot_0`, `rot_1`, `rot_2`, `rot_3` | 4 | Quaternion (w, x, y, z) |

**Détection** : présence de `f_dc_0` OU `opacity` OU `scale_0` dans le header PLY.

Formats compressés disponibles mais pas exportés nativement par HY-World :
- `.spz` (Niantic) : ~10x plus compact que PLY
- `.ksplat` (PlayCanvas)

#### Format `camera_params.json`

```json
{
  "extrinsics": [
    { "camera_id": 0, "matrix": [[4x4 c2w float]] }
  ],
  "intrinsics": [
    { "camera_id": 0, "matrix": [[3x3 intrinsic float]] }
  ]
}
```

- Convention : **OpenCV** (pas OpenGL)
- Extrinsics = camera-to-world, normalisés sur la première frame
- **Conversion requise pour Bevy** : Bevy utilise right-handed Y-up, -Z forward (OpenGL) → flip obligatoire

#### Lighting

Aucun export de métadonnées lumière. L'IBL (Image-Based Lighting) est intégré dans la scène 3DGS par WorldLens — non exportable séparément.

### 1.4 Dépendances Python notables

| Crate/Lib | Version | Rôle |
|---|---|---|
| `gsplat` | 1.5.3 | Rasterizer 3DGS (nerfstudio fork) |
| `open3d` | 0.18.0 | Traitement point cloud |
| `trimesh` | — | Traitement mesh |
| `pycolmap` | 3.10.0 | Reconstruction COLMAP |
| `torch` | 2.4.0 + CUDA 12.4 | Inférence |
| `onnxruntime` | 1.19.2 (GPU Win) | Sky segmentation |

### 1.5 État de `bevy_gaussian_splatting`

| Champ | Valeur |
|---|---|
| Version | **7.0.1** (février 2026) |
| Bevy compatible | **0.18** ✅ |
| GitHub | `mosure/bevy_gaussian_splatting` |
| Stars | 255 (communauté petite) |
| Issues ouvertes | 81 |
| Documentation | 0% (lire le source) |

**Attention nightly** : la feature `nightly_generic_alias` est activée par défaut. Forgia étant sur stable Rust, il faut :
```toml
bevy_gaussian_splatting = { version = "7", default-features = false }
```

**Formats supportés** : `.ply`, `.gcloud` (natif bincode2), `.spz` (Niantic), `.glb/.gltf` (KHR_gaussian_splatting RC)

**Issues bloquantes à surveiller** :
- #208 : compression SOGS/JPEG pas encore supportée
- #225 : API SH degree comme const generic — risque de churn

### 1.6 Alternatives à `bevy_gaussian_splatting`

| Option | Langage | Bevy compat | Statut | Usage recommandé |
|---|---|---|---|---|
| `bevy_gaussian_splatting` 7.0.1 | Rust | 0.18 ✅ | Actif | **Recommandé** — seule option native Bevy |
| `wgpu-3dgs-viewer` 0.6 | Rust/wgpu | Non | Actif | Processus externe / viewer standalone |
| `web-splat` | Rust/wgpu | Non | Abandonné (2024) | Non |
| gsplat (Python/CUDA) | Python | Non | Actif (nerfstudio) | Côté serveur Python uniquement |
| gsplat Rust port | — | — | **N'existe pas** | — |

**Conclusion faisabilité technique** : intégration possible via `bevy_gaussian_splatting` 7.0.1 pour le rendu. La génération d'assets nécessite un sidecar Python (GPU, CUDA 12.4, ~10-20s par scène). Le pipeline complet (génération texte→monde) n'est pas encore disponible.

---

## Étape 2 — Audit de licence

### 2.1 Identification

**Nom complet** : Tencent HY-WORLD 2.0 Community License Agreement  
**Date** : 15 avril 2026  
**Type** : Licence propriétaire communautaire (NON MIT, NON Apache, NON LGPL)  
**Droit applicable** : Droit de Hong Kong SAR, juridictions HK

### 2.2 Tableau des clauses critiques

| Clause | Contenu | Impact Forgia |
|---|---|---|
| **Territoire** | **Exclut explicitement EU, UK et Corée du Sud** | ⛔ BLOQUANT si Forgia déployé/hébergé en UE |
| **Seuil MAU** | >1M MAU au 15/04/2026 → licence séparée à demander | Non bloquant actuellement |
| **Usage commercial** | Autorisé sous 1M MAU | OK à court terme |
| **Outputs générés** | Tencent ne revendique aucun droit | Bon : les mondes générés appartiennent au créateur |
| **Restriction outputs** | Interdit d'utiliser les outputs pour entraîner d'autres modèles IA (hors Hunyuan) | Impacte tout pipeline ML downstream |
| **Distribution** | Copie de la licence obligatoire + "Powered by Tencent HY" encouragé | Contrainte UX/marketing |
| **Garanties** | Aucune (AS IS) | Standard |
| **Responsabilité** | Entièrement dégagée | Standard |

### 2.3 Réponses directes aux questions

**Usage commercial autorisé ?**
→ **Oui**, sous 1M MAU. Au-delà, contacter hunyuan3d@tencent.com pour une licence commerciale.

**Redistribution des assets générés ?**
→ **Oui**, sans restriction de Tencent. Les assets générés appartiennent à l'utilisateur.

**Clauses de revshare ou non-concurrence ?**
→ **Non** de revshare. **Non** de clause de non-concurrence explicite. Mais : l'interdiction d'utiliser les outputs pour entraîner d'autres modèles est une contrainte indirecte sur les pipelines ML (ex: fine-tuning d'un modèle de génération Forgia à partir d'assets HY-World = interdit).

**Implications pour une plateforme UGC où les créateurs monétisent leurs mondes ?**
→ 3 points :
1. **EU/UK** : si des créateurs ou serveurs sont en UE, la licence ne couvre pas leur usage → **risque juridique réel**.
2. **Pipeline ML** : si Forgia veut apprendre sur les mondes générés par HY-World pour améliorer sa propre IA → **interdit**.
3. **Dépendance** : Tencent peut modifier les termes ou couper l'accès aux poids → **risque de continuité**.

### 2.4 Verdict licence

| Critère | Résultat |
|---|---|
| Utilisable en prod (France, hors EU) | Oui (France = UE, donc NON) |
| Utilisable en prod (US, Canada, Asie) | Oui sous 1M MAU |
| Redistribution assets joueurs | Oui |
| Pipeline ML sur outputs | Non |
| Risque pour plateforme UGC EU | **Bloquant** |

> **Recommandation** : avant toute intégration en production sur une plateforme EU, contacter Tencent à hunyuan3d@tencent.com pour négocier un addendum territorial. En attendant, l'usage en R&D interne (non déployé) est dans une zone grise exploitable.

---

## Résumé exécutif

| Axe | Status |
|---|---|
| Formats d'export | Bien documentés, parsables en Rust |
| Pipeline dispo | Reconstruction seule (génération = coming soon) |
| Bevy 0.18 compat | Via `bevy_gaussian_splatting` 7.0.1 |
| Conversion d'axes | Requise (OpenCV → Bevy) |
| Licence commerciale EU | **Non couverte** — risque juridique |
| Restriction ML pipeline | Oui |
| Maturité globale | Faible (13 jours, pipeline incomplet) |

**Feu vert pour prototype R&D interne** : Oui, sous réserve de ne pas déployer en EU avant clarification licence.  
**Feu vert pour intégration production plateforme UGC EU** : Non sans addendum Tencent.
