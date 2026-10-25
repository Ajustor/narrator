#!/usr/bin/env bash
set -euo pipefail

# Télécharge le modèle Qwen2.5-3B-Instruct (GGUF, Q4_K_M, ~1,9 Go) s'il est
# absent, puis lance le prototype console avec NARRATEUR_MODELE pointant
# vers lui.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELES_DIR="$REPO_ROOT/modeles"
MODELE_FICHIER="Qwen2.5-3B-Instruct-Q4_K_M.gguf"
MODELE_CHEMIN="$MODELES_DIR/$MODELE_FICHIER"
MODELE_URL="https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/$MODELE_FICHIER"

if ! command -v cmake >/dev/null 2>&1; then
    echo "cmake est requis pour compiler llama-cpp-2 et n'est pas trouvable sur le PATH." >&2
    echo "Installe-le, par exemple :" >&2
    echo "  sudo apt install cmake" >&2
    echo "ou, sans sudo, via un venv Python :" >&2
    echo "  python3 -m venv ~/.local/venvs/cmake && ~/.local/venvs/cmake/bin/pip install cmake && mkdir -p ~/.local/bin && ln -s ~/.local/venvs/cmake/bin/cmake ~/.local/bin/cmake" >&2
    exit 1
fi

mkdir -p "$MODELES_DIR"

if [ ! -f "$MODELE_CHEMIN" ]; then
    echo "Modèle introuvable, téléchargement de $MODELE_FICHIER (~1,9 Go)..."
    curl -L --fail --progress-bar -o "$MODELE_CHEMIN.tmp" "$MODELE_URL"
    mv "$MODELE_CHEMIN.tmp" "$MODELE_CHEMIN"
    echo "Téléchargement terminé : $MODELE_CHEMIN"
else
    echo "Modèle déjà présent : $MODELE_CHEMIN"
fi

echo "Lancement du jeu (NARRATEUR_MODELE=$MODELE_CHEMIN)..."
cd "$REPO_ROOT"
NARRATEUR_MODELE="$MODELE_CHEMIN" exec cargo run --release -p app
