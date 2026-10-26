#!/usr/bin/env bash
set -euo pipefail

# Remplace l'icône Android par défaut (gabarit "robot vert" d'Android
# Studio) par l'icône de l'app, à partir de crates/app_desktop/assets/icon.png.
#
# Nécessaire car `dx` 0.7.9 n'implémente pas la génération d'icône Android :
# `[bundle] icon`/`[android] icon` dans Dioxus.toml sont lus mais jamais
# appliqués au bundle Android, et `dx build`/`dx bundle` ré-écrit
# systématiquement le gabarit par défaut à CHAQUE invocation (y compris en
# incrémental) — confirmé empiriquement : un patch manuel posé avant un
# second `dx bundle` était annulé par ce second appel. Ce script doit donc
# être relancé après chaque `dx build`/`dx bundle`, puis le paquet doit être
# (re)signé via `./gradlew assembleDebug` directement (pas via `dx`), pour
# que le patch survive jusqu'à l'APK final. Voir les instructions
# d'installation pour la séquence complète.
#
# Usage : scripts/patch_icone_android.sh [chemin-du-projet-android]
# Par défaut : target/dx/Narrator/debug/android/app (celui généré par
# `dx build --platform android --target aarch64-linux-android ...` depuis
# crates/app_desktop, avec le binaire renommé "Narrator").

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONE_SOURCE="$REPO_ROOT/crates/app_desktop/assets/icon.png"
PROJET_ANDROID="${1:-$REPO_ROOT/target/dx/Narrator/debug/android/app}"
RES="$PROJET_ANDROID/app/src/main/res"

if [ ! -f "$ICONE_SOURCE" ]; then
    echo "icône source introuvable : $ICONE_SOURCE" >&2
    exit 1
fi
if [ ! -d "$RES" ]; then
    echo "projet Android introuvable : $RES (lance d'abord dx build --platform android)" >&2
    exit 1
fi
if ! command -v convert >/dev/null 2>&1; then
    echo "ImageMagick (commande 'convert') est requis." >&2
    exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Glyphe seul (anneau + N), fond transparent — sert de calque "foreground"
# de l'icône adaptative (API 26+) : le fond solide est fourni séparément
# par le vecteur ci-dessous, pas question de dupliquer le fond dans ce
# calque (zone de sécurité de l'icône adaptative : le contenu doit tenir
# dans ~66 % du canevas 108dp pour ne pas être rogné par le masque du
# lanceur).
convert -size 1024x1024 xc:none \
    -fill none -stroke "#c9a227" -strokewidth 26 \
    -draw "circle 512,512 512,70" \
    -font /usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf \
    -fill "#c9a227" -gravity center -pointsize 580 \
    -annotate +0+20 "N" \
    "$TMP/glyphe.png"
convert -size 432x432 xc:none \( "$TMP/glyphe.png" -resize 280x280 \) -gravity center -composite "$TMP/foreground.png"

mkdir -p "$RES/drawable"
cat > "$RES/drawable/ic_launcher_background.xml" <<'EOF'
<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:fillColor="#161310"
        android:pathData="M0,0h108v108h-108z" />
</vector>
EOF

rm -f "$RES/drawable-v24/ic_launcher_foreground.xml"
rmdir "$RES/drawable-v24" 2>/dev/null || true
cp "$TMP/foreground.png" "$RES/drawable/ic_launcher_foreground.png"

for dp_taille in "mdpi:48" "hdpi:72" "xhdpi:96" "xxhdpi:144" "xxxhdpi:192"; do
    dp="${dp_taille%%:*}"
    taille="${dp_taille##*:}"
    rm -f "$RES/mipmap-$dp/ic_launcher.webp"
    convert "$ICONE_SOURCE" -resize "${taille}x${taille}" "$RES/mipmap-$dp/ic_launcher.png"
done

echo "Icône appliquée dans $RES."
echo "Reconstruis maintenant l'APK SANS repasser par dx (qui réécraserait ce patch) :"
echo "  cd \"$PROJET_ANDROID\" && ./gradlew assembleDebug"
