#!/usr/bin/env bash
set -euo pipefail

# Déclare le `signingConfig` de release dans le projet Android généré par
# `dx`, à partir d'un keystore et de ses identifiants passés par variables
# d'environnement — jamais en dur dans ce script ni dans le dépôt.
#
# Nécessaire pour la même raison que les deux scripts voisins
# (`patch_icone_android.sh`, `patch_manifest_maj_android.sh`) : `dx`
# régénère `build.gradle.kts` à CHAQUE `dx build`/`dx bundle` sans jamais
# conserver de `signingConfig` sur le buildType `release` — sans lui,
# `./gradlew assembleRelease` retombe sur une signature de debug générée à
# la volée, différente à chaque poste/CI, ce qui rendrait toute mise à jour
# incompatible avec l'APK déjà installée (Android refuse d'installer par-
# dessus une signature différente).
#
# Variables d'environnement requises :
#   ANDROID_KEYSTORE_PATH     chemin vers le fichier .keystore (PKCS12)
#   ANDROID_KEYSTORE_PASSWORD mot de passe du keystore
#   ANDROID_KEY_ALIAS         alias de la clé à l'intérieur du keystore
#   ANDROID_KEY_PASSWORD      mot de passe de la clé (souvent identique au
#                              précédent : un keystore PKCS12 n'autorise pas
#                              deux mots de passe différents)
#
# Usage : scripts/patch_signing_android.sh [chemin-du-projet-android]
# Par défaut : target/dx/Narrator/release/android/app (celui généré par
# `dx bundle --platform android --release ...` depuis crates/app_desktop).
# Idempotent : le relancer sur un projet déjà patché remplace simplement le
# bloc existant.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJET_ANDROID="${1:-$REPO_ROOT/target/dx/Narrator/release/android/app}"
GRADLE="$PROJET_ANDROID/app/build.gradle.kts"

if [ ! -f "$GRADLE" ]; then
    echo "projet Android introuvable : $GRADLE (lance d'abord dx bundle --platform android)" >&2
    exit 1
fi

for variable in ANDROID_KEYSTORE_PATH ANDROID_KEYSTORE_PASSWORD ANDROID_KEY_ALIAS ANDROID_KEY_PASSWORD; do
    if [ -z "${!variable:-}" ]; then
        echo "variable d'environnement manquante : $variable" >&2
        exit 1
    fi
done
if [ ! -f "$ANDROID_KEYSTORE_PATH" ]; then
    echo "keystore introuvable : $ANDROID_KEYSTORE_PATH" >&2
    exit 1
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# Retire un éventuel bloc `signingConfigs { ... }` et toute ligne
# `signingConfig = ...` posés par un passage précédent de ce script, pour
# rester idempotent plutôt que d'empiler des blocs dupliqués.
python3 - "$GRADLE" "$TMP" <<'PYEOF'
import re
import sys

chemin_source, chemin_tmp = sys.argv[1], sys.argv[2]
contenu = open(chemin_source, encoding="utf-8").read()
# Le bloc posé par ce script a toujours exactement cette forme (un seul
# niveau imbriqué, `create("release") { ... }`) : on exige les DEUX
# fermetures de bloc à la suite pour ne pas s'arrêter sur la première
# accolade fermante rencontrée (celle de `create`, plus indentée) et
# laisser la fermeture de `signingConfigs` elle-même pendre derrière.
contenu = re.sub(
    r'\n *signingConfigs \{\n *create\("release"\) \{.*?\n *\}\n *\}\n',
    "\n",
    contenu,
    flags=re.DOTALL,
)
contenu = re.sub(r"\n *signingConfig = signingConfigs\.getByName\([^)]*\)\n", "\n", contenu)
open(chemin_tmp, "w", encoding="utf-8").write(contenu)
PYEOF

# Réinjecte un bloc `signingConfigs { create("release") { ... } }` dans
# `android { ... }` (juste avant `buildTypes`) et l'assigne au buildType
# `release`. Les valeurs viennent des variables d'environnement, jamais
# écrites en dur.
python3 - "$TMP" "$GRADLE" <<PYEOF
import re
import sys

chemin_tmp, chemin_dest = sys.argv[1], sys.argv[2]
contenu = open(chemin_tmp, encoding="utf-8").read()

bloc_signing = '''    signingConfigs {
        create("release") {
            storeFile = file("${ANDROID_KEYSTORE_PATH}")
            storePassword = "${ANDROID_KEYSTORE_PASSWORD}"
            keyAlias = "${ANDROID_KEY_ALIAS}"
            keyPassword = "${ANDROID_KEY_PASSWORD}"
        }
    }

'''

contenu, nb = re.subn(r"( *buildTypes \{)", bloc_signing + r"\1", contenu, count=1)
if nb != 1:
    sys.exit("balise buildTypes introuvable dans " + chemin_dest)

contenu, nb = re.subn(
    r'( *getByName\("release"\) \{\n)',
    r'\1            signingConfig = signingConfigs.getByName("release")\n',
    contenu,
    count=1,
)
if nb != 1:
    sys.exit('bloc getByName("release") introuvable dans ' + chemin_dest)

open(chemin_dest, "w", encoding="utf-8").write(contenu)
PYEOF

echo "signingConfig de release posé dans $GRADLE (alias $ANDROID_KEY_ALIAS)."
