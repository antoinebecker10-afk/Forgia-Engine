# Story-588 — Compression textures KTX2/UASTC (VRAM ÷4)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `material_override.rs`, symbole `Bc7RgbaUnormSrgb`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : en cours (2026-06-09). Bug B4 de l'audit A→Z monde RPG.
> **Scope BMAD** : Standard+ (workspace Cargo.toml feature + assets + load paths). Vertical slice bark d'abord.

## Problème (audit B4)

VRAM ~886 MB, dont ~872 MB de textures **brutes RGBA8** (décodées depuis .jpg). Pire offender : `jolcham_oak_bark_01` (3× 2K, 7–10 MB chacune sur disque, chargées **inconditionnellement au Startup** dans `forgia-foliage/material_override.rs:142` `preload_bark_textures`). Décodées RGBA8 + mips ≈ 64 MB rien que pour le bark. 886 MB ≠ crash sur GPU moderne mais gros gaspillage + upload GPU lent (impacte aussi le chargement).

## Approche : UASTC + transcode BC7 (desktop)

- Encodeur : **KTX-Software 4.4.2** (`toktx`/`ktx`) installé dans `C:\Users\Antoi\tools\KTXSoftware\` (hors PATH, hors repo). Pas sur winget → release GitHub officielle Khronos, install silencieuse NSIS `/S /D=` (dossier user, pas d'admin).
- KTX-Software ne sait pas encoder du **BC7 brut**, seulement **UASTC / ETC1S / ASTC**. UASTC transcode en BC7 sur desktop (qualité quasi-lossless) → **VRAM ÷4** (BC7 = 1 byte/px vs RGBA8 4 byte/px).
- Côté bevy : `ktx2` déjà activé. Il faut **ajouter `basis-universal`** (transcode UASTC→BC7 au load). C++ via `cc` → VC.Tools présents (vswhere OK). `zstd` non requis pour le gain VRAM (UASTC uncompressed sur disque ≈ plus petit que les jpg sources) — différé.
- **Color space** : bevy choisit le format de transcode BC7 via `ImageLoaderSettings.is_srgb` (ktx2.rs `Bc7RgbaUnormSrgb` vs `Bc7RgbaUnorm`), **pas** l'OETF du fichier (ignoré pour UASTC). L'ancien code chargeait les 3 jpg avec `is_srgb=true` (défaut du closure `repeat`) → bark validé en **tout-sRGB**. Pour tenir la promesse « aucune perte visible », on **préserve tout-sRGB** (3 textures encodées sRGB, closure inchangé). Corriger nor/arm en linéaire (PBR strict) changerait l'éclairage du bark → **follow-up qualité séparé**, hors scope VRAM.

## Implémentation (incrémentale)

1. **Vertical slice bark** : encoder les 3 textures `jolcham_oak_bark_01` en `.ktx2` UASTC + mips, re-câbler `BARK_*_PATH` (.jpg→.ktx2) dans `material_override.rs`, activer `basis-universal`, valider runtime (visuel bark + VRAM).
2. **Extension** (après validation slice) : armes (2K), autres textures lourdes terrain/déco.

## Critères d'acceptation

- AC1 — 3 bark `.ktx2` UASTC valides (ktxinfo : UASTC, 13 mips, 2048×4096 préservé ; ktx2check 0 erreur). ✅
- AC2 — `BARK_*_PATH` pointent les `.ktx2`, build OK avec `basis-universal` (73 crates, C++ compilé). ✅ (load runtime à confirmer)
- AC3 — Bark visuellement **identique à avant** (tout-sRGB préservé), pas d'artefacts de blocs BC7 visibles. ⏳ runtime
- AC4 — VRAM bark ÷~4 (~128 MB → ~32 MB), sensor VRAM si dispo. ⏳ runtime
- AC5 — `cargo check -p forgia` + clippy 0 (forgia-foliage). ✅

## Vigilance

- Feature `basis-universal` = **rebuild workspace complet** (change Cargo.toml workspace). Coordination multi-terminal : workspace Cargo.toml ≠ fichiers de l'autre session (qui touche forgia-game/Cargo.toml, pas le workspace root).
- Si transcode KO sur la GPU cible (rare desktop) → fallback ASTC (mobile) ou garder jpg.
- Outil hors repo (`C:\Users\Antoi\tools\KTXSoftware\`) → documenter pour repro (pas commité).
