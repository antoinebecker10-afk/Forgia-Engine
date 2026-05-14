# Forgia V2 — macOS SDK

Place `MacOSX26.2.sdk.tar.bz2` ici (gitignored, pas commitable).

## Pourquoi ce dossier

Le `engine-builder/Dockerfile` cross-compile macOS et iOS depuis Linux via `osxcross`. Cette toolchain a besoin du SDK officiel Apple. Apple ne distribue PAS publiquement les SDK — il faut le générer depuis ton install Xcode.

## Génération depuis Xcode (Mac requis)

Sur un Mac avec Xcode 16+ installé :

```bash
git clone --depth 1 https://github.com/tpoechtrager/osxcross /tmp/osxcross
cd /tmp/osxcross
XCODEDIR=/Applications/Xcode.app/Contents/Developer ./tools/gen_sdk_package.sh
mv MacOSX26.2.sdk.tar.bz2 /path/to/Forgia\ Rewrite/docker/sdk/
```

Puis sur la machine Linux où tu builds Docker :

```bash
docker build -f docker/engine-builder/Dockerfile -t forgia/engine-builder:0.1.0 .
```

## Deployment target

`MACOSX_DEPLOYMENT_TARGET=14.0` dans le Dockerfile. Les binaires compilés contre ce SDK tournent sur **macOS 14+** (Sonoma et plus récent).

## Si pas de Mac dispo

Tu peux skip le SDK macOS et builder uniquement Windows/Linux/WASM. Modifier le `Dockerfile engine-builder` pour retirer la section `osxcross` et les targets `*-apple-*`.

Ship V1 Bots Brawl Q4 2026 = Windows uniquement → SDK macOS pas requis avant V2.M2.
