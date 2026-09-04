# Forgia V2 — Docker infrastructure

> Pattern aligné Renzora (`renzora/engine` 2026-03). 2 Dockers de production + 1 dossier SDK.

## Structure

```
docker/
├── engine-builder/     Dockerfile — cross-platform toolchain (compile Forgia)
├── plugin-builder/     Dockerfile — marketplace build server (compile plugins community)
├── sdk/                README — placement du SDK macOS (osxcross)
└── README.md           ce fichier
```

## Usage

### engine-builder — Cross-platform compile

Image Docker qui compile `forgia-game` pour TOUTES les plateformes depuis Linux. Pas de source code dans l'image, le repo est monté à runtime.

**Targets** : Linux (gnu) / Windows (mingw) / macOS Intel + Apple Silicon / iOS + iOS sim / Android arm64+x86_64 / WASM.

```bash
# Build l'image (~30 min one-shot)
docker build -f docker/engine-builder/Dockerfile -t forgia/engine-builder:0.1.0 .

# Container persistant + build all platforms
docker run -d --name forgia-builder -v "$(pwd):/app/src" forgia/engine-builder:0.1.0
docker exec forgia-builder bash -c "cd /app/src && cargo build -p forgia-game --release --target x86_64-pc-windows-gnu"
```

### plugin-builder — Marketplace community plugins

Image Docker qui compile les plugins user contre la dylib pré-buildée du moteur. Pour la marketplace V2-V3.

**Targets** : Windows `.dll` / Linux `.so` / macOS `.dylib`.

```bash
# Build l'image (~10-20 min, une fois par release moteur)
docker build -f docker/plugin-builder/Dockerfile -t forgia/plugin-builder:0.1.0 .

# Build un plugin community (~5-10 sec)
docker run --rm \
  -v /path/to/plugin/source:/plugin \
  -v /path/to/output:/output \
  forgia/plugin-builder:0.1.0 \
  /app/scripts/build-plugin.sh /plugin /output
```

### sdk — macOS SDK pour cross-compile

Voir `sdk/README.md`. Place `MacOSX26.2.sdk.tar.bz2` dans ce dossier avant de build l'image (extraite via osxcross depuis Xcode).

## Status V2

| Phase | Usage Docker |
|---|---|
| **Phase 1-2** (Hello World + Gunfeel) | Pas utilisé. CI Windows GitHub Actions suffit pour ship V1. |
| **Phase 6** (Steam ship) | Optionnel — `engine-builder` si build Linux + macOS demandé Steam |
| **V2 M2** (Editor public) | `engine-builder` actif pour cross-platform. |
| **V2 M3** (UGC Hub Marketplace) | `plugin-builder` actif. Les contributeurs publient via cette infra. |

## Inspiration

Pattern direct depuis `renzora/engine` (alpha r1-alpha4, 2026-03-30, MIT/Apache). Adapté Forgia : naming `forgia-` au lieu de `renzora-`, build target `forgia-game` au lieu de `renzora-runtime`.
