#!/usr/bin/env bash
set -euo pipefail

# Déclare le `FileProvider` nécessaire à l'installation des mises à jour
# Android (`mise_a_jour::lancer_installation`) dans le projet Android généré
# par `dx` : ajoute le `<provider>` dans `<application>` de
# AndroidManifest.xml et crée `res/xml/file_paths.xml`, qui expose le
# répertoire `<getFilesDir()>/maj/` où `worker::chemin_apk_mise_a_jour`
# télécharge l'APK.
#
# Nécessaire car `dx` 0.7.9 n'offre aucun moyen déclaratif d'y parvenir —
# confirmé empiriquement (et en lisant le gabarit de manifeste embarqué
# dans le binaire `dx`) :
#   - `[android.raw] application = "..."` (« Raw XML inside <application> »
#     d'après `dx config schema`) est lu mais SILENCIEUSEMENT ignoré : le
#     gabarit de AndroidManifest.xml n'a aucun emplacement pour ce champ ;
#   - `[android.raw] manifest` n'injecte qu'en dehors de `<application>`,
#     or un `<provider>` doit être dedans ;
#   - aucun champ ne permet d'ajouter une ressource Android (`res/xml/...`)
#     — `resources` va dans les assets, pas dans `res/`.
# Ce qui, en revanche, FONCTIONNE déclarativement et reste donc dans
# Dioxus.toml : la permission `REQUEST_INSTALL_PACKAGES`
# (`[android.permissions]`) et la règle R8 qui conserve
# `FileProvider.getUriForFile` (`proguard_rules`).
#
# Comme `scripts/patch_icone_android.sh`, `dx build`/`dx bundle` régénère
# le manifeste à CHAQUE invocation : ce script doit être relancé après
# chaque `dx build`/`dx bundle`, puis le paquet reconstruit via
# `./gradlew assembleDebug`/`assembleRelease` directement (pas via `dx`).
# Il est idempotent : le relancer sur un projet déjà patché ne change rien.
#
# Usage : scripts/patch_manifest_maj_android.sh [chemin-du-projet-android]
# Par défaut : target/dx/Narrator/debug/android/app (celui généré par
# `dx build --platform android --target aarch64-linux-android ...` depuis
# crates/app_desktop, avec le binaire renommé "Narrator").

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJET_ANDROID="${1:-$REPO_ROOT/target/dx/Narrator/debug/android/app}"
MAIN="$PROJET_ANDROID/app/src/main"
MANIFESTE="$MAIN/AndroidManifest.xml"
RES="$MAIN/res"

if [ ! -f "$MANIFESTE" ]; then
    echo "projet Android introuvable : $MANIFESTE (lance d'abord dx build --platform android)" >&2
    exit 1
fi

# Chemin exposé : doit rester aligné sur `worker::chemin_apk_mise_a_jour`
# (`repertoire_donnees_app().join("maj").join("narrateur.apk")`, où
# `repertoire_donnees_app()` est `Activity.getFilesDir()`) — d'où un
# `<files-path>` sur le sous-répertoire `maj/`, et rien de plus large.
mkdir -p "$RES/xml"
cat > "$RES/xml/file_paths.xml" <<'EOF'
<?xml version="1.0" encoding="utf-8"?>
<paths>
    <files-path name="maj" path="maj/" />
</paths>
EOF

if grep -q 'androidx.core.content.FileProvider' "$MANIFESTE"; then
    echo "FileProvider déjà déclaré dans $MANIFESTE."
else
    # Autorité `${applicationId}.fileprovider` : placeholder résolu par
    # Gradle à la fusion du manifeste ; `lancer_installation` reconstruit la
    # même valeur via `getPackageName()`.
    PROVIDER='        <provider\
            android:name="androidx.core.content.FileProvider"\
            android:authorities="${applicationId}.fileprovider"\
            android:exported="false"\
            android:grantUriPermissions="true">\
            <meta-data\
                android:name="android.support.FILE_PROVIDER_PATHS"\
                android:resource="@xml/file_paths" />\
        </provider>'
    if [ "$(grep -c '</application>' "$MANIFESTE")" -ne 1 ]; then
        echo "balise </application> introuvable ou multiple dans $MANIFESTE" >&2
        exit 1
    fi
    sed -i "s#^\(\s*\)</application>#$PROVIDER\n\1</application>#" "$MANIFESTE"
    if ! grep -q 'androidx.core.content.FileProvider' "$MANIFESTE"; then
        echo "échec de l'injection du FileProvider dans $MANIFESTE" >&2
        exit 1
    fi
fi

echo "FileProvider de mise à jour déclaré dans $PROJET_ANDROID."
echo "Reconstruis maintenant l'APK SANS repasser par dx (qui réécraserait ce patch) :"
echo "  cd \"$PROJET_ANDROID\" && ./gradlew assembleDebug   # ou assembleRelease"
