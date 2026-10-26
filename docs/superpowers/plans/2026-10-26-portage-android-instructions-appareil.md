# Instructions d'installation du port Android - Narrateur

## Résumé

Le port Android de Narrateur compile et produit une APK de debug pour
**arm64-v8a** (le processeur des téléphones Android récents). Ce document
donne les instructions pour l'installer et la tester sur un appareil réel.

Tous les chiffres ci-dessous sont relevés sur l'APK réellement construite
(`aapt2 dump badging`, `unzip -l`), pas estimés.

## Commande de construction

L'APK a été produite depuis `crates/app_desktop` avec :

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export ANDROID_NDK_HOME=/home/alexandre/Android/Sdk/ndk/29.0.14206865
export ANDROID_HOME=/home/alexandre/Android/Sdk
cd crates/app_desktop
dx build --platform android --target aarch64-linux-android --package app_desktop \
    --no-default-features --features plateforme-android,document
```

`--target aarch64-linux-android` est **obligatoire** : sans lui, `dx`
construit pour l'architecture de la machine hôte (x86_64), et l'APK
obtenue ne s'installe pas sur un téléphone.

## Chemin de l'APK

Relatif à la racine du dépôt :

```
target/dx/app_desktop/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Soit, en absolu depuis le dépôt principal (après la fusion de la branche `portage-android` dans `master`, le worktree de développement a été supprimé — c'est désormais le seul emplacement) :

```
/home/alexandre/dev/narrator/target/dx/app_desktop/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Taille : 96 112 216 octets (≈ 92 Mio).

Contenu natif (`unzip -l`) :

```
  9290184  lib/arm64-v8a/libc++_shared.so
285338312  lib/arm64-v8a/libmain.so
```

## Identité et compatibilité de l'application

| Propriété | Valeur réelle |
| --- | --- |
| Identifiant (`applicationId`) | `com.alexandregambier.narrateur` |
| Nom affiché sur l'appareil | **AppDesktop** (voir note ci-dessous) |
| Activité lancée | `dev.dioxus.main.MainActivity` |
| Bibliothèque native chargée | `libmain.so` (nom logique : `main`) |
| `minSdkVersion` | **24** — Android 7.0 (Nougat) et plus |
| `targetSdkVersion` | **34** — Android 14 |
| `compileSdkVersion` | 34 |
| `versionCode` / `versionName` | 1 / 0.1.0 |
| Architecture (`native-code`) | **arm64-v8a uniquement** |
| Permissions | `android.permission.INTERNET` (ajoutée par défaut par `dx`, nécessaire au téléchargement du modèle) |

- **Architecture** : l'APK ne contient que `arm64-v8a`. Elle s'installe sur
  tout téléphone Android 64 bits récent, mais **pas** sur un émulateur
  x86_64 — pour l'émulateur, reconstruire avec
  `--target x86_64-linux-android`.
- **Nom affiché** : l'identifiant est désormais stable et propre (fixé dans
  `crates/app_desktop/Dioxus.toml`), ce qui permet la mise à jour en place
  (`adb install -r`). Le libellé visible sur l'écran d'accueil reste
  « AppDesktop » : `dx` 0.7.9 le dérive du nom de l'exécutable Cargo
  (`app_desktop`) et ignore `[application] name` pour Android. C'est donc
  sous ce nom qu'il faut chercher l'icône après l'installation.

## Installation sur un appareil Android

### Prérequis

- Un appareil Android 7.0+ (API 24+) **64 bits (arm64)** connecté en USB
- Le débogage USB activé sur l'appareil
- `adb` installé et configuré sur la machine de développement
  (`adb devices` doit lister l'appareil)
- Une connexion réseau stable sur l'appareil (pour le téléchargement du modèle)
- Au minimum 3 Go d'espace de stockage libre sur l'appareil

### Commande d'installation

**Cette commande suppose que le répertoire courant du shell est la racine
du dépôt** (`/home/alexandre/dev/narrator`) ; sinon, utiliser le chemin
absolu donné plus haut.

```bash
cd /home/alexandre/dev/narrator
adb install -r target/dx/app_desktop/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Le drapeau `-r` réinstalle par-dessus une version déjà présente en
conservant les données (donc le modèle déjà téléchargé).

## Premier lancement et téléchargement du modèle

**Important** : au premier lancement, l'application télécharge
automatiquement le modèle de langage
`Qwen2.5-3B-Instruct-Q4_K_M.gguf` — **1 929 903 264 octets, soit ≈ 1,8 Gio**
(taille relevée directement sur l'URL source).

Le fichier est écrit dans le stockage privé de l'application
(`Activity.getFilesDir()`), c'est-à-dire
`/data/user/0/com.alexandregambier.narrateur/files/Qwen2.5-3B-Instruct-Q4_K_M.gguf`.

### Points à noter

- Le téléchargement prend plusieurs minutes selon la connexion (WiFi fortement recommandé)
- L'appareil doit rester branché et l'écran allumé pendant le téléchargement
- L'écran de téléchargement apparaît dès l'ouverture de la connexion, même
  si le serveur n'annonce pas la taille du fichier
- Le modèle n'est téléchargé qu'une fois : aux lancements suivants,
  l'application démarre directement
- **Pas de reprise** : si le téléchargement échoue ou est interrompu
  (réseau coupé, application tuée), relancer l'application **recommence le
  téléchargement depuis zéro**. Le fichier partiel `.tmp` est supprimé
  automatiquement sur échec, donc l'espace disque est bien restitué — mais
  il faut retélécharger les ~1,8 Gio.
- Un réseau bloqué fait échouer le téléchargement au bout de 30 s
  (délais de connexion et de lecture) au lieu de figer l'application
  indéfiniment

## Vérification du fonctionnement sur l'appareil réel

Après installation et lancement, vérifier visuellement :

- [ ] L'application démarre sans crash
- [ ] **Écran de téléchargement** : un écran s'affiche avec une barre de progression visible
- [ ] **Progression** : la barre avance et le pourcentage augmente
- [ ] **Transition post-téléchargement** : à la fin, l'application passe à l'écran de création de personnage
- [ ] **Écran de création** : l'interface correspond à celle de la version desktop
- [ ] **Interface générale** : navigation et mise en forme identiques à la version desktop
- [ ] **Fonctionnalités de base** : création d'un personnage et premiers tours de jeu sans crash
- [ ] **Échec de téléchargement** : si le réseau est coupé pendant le
      téléchargement, un écran d'erreur s'affiche avec un conseil
      *réseau/stockage* (et non une instruction sur `NARRATEUR_MODELE`,
      qui n'existe pas sur Android) ; relancer l'application redémarre le
      téléchargement depuis zéro

## Notes techniques

- L'APK est une build de **debug** : symboles de débogage inclus, donc
  volumineuse (90 Mio) et plus lente qu'une build de release
- La bibliothèque native s'appelle `libmain.so` (et non `libapp_desktop.so`) :
  c'est le nom attendu par la `NativeActivity` de Dioxus
- `libc++_shared.so` doit être présente à côté de `libmain.so` dans l'APK
  (`libmain.so` en dépend via `DT_NEEDED`) ; si un rebuild incrémental la
  fait disparaître, supprimer `target/dx/app_desktop/debug/android` et
  reconstruire proprement

## Signalement de problèmes

1. Vérifier que `adb devices` affiche bien l'appareil
2. Consulter les logs. Le filtre par nom de bibliothèque ne fonctionne pas
   (la bibliothèque s'appelle `main`, pas `app_desktop`) ; utiliser plutôt :

   ```bash
   # tous les logs pertinents, quel que soit le tag
   adb logcat | grep -iE 'dioxus|RustStdoutStderr|panic|narrateur'

   # ou, plus précis : uniquement le processus de l'application
   adb logcat --pid=$(adb shell pidof -s com.alexandregambier.narrateur)
   ```

   Les `eprintln!` du code Rust (dont la trace
   « modèle absent à …, téléchargement en cours depuis … ») remontent dans
   `logcat` via le tag `RustStdoutStderr`.
3. En cas d'échec d'installation avec `INSTALL_FAILED_NO_MATCHING_ABIS` :
   l'appareil (ou l'émulateur) n'est pas arm64 — reconstruire avec le
   `--target` correspondant
4. Vérifier que le débogage USB est bien activé
5. Réinstaller avec `-r` si l'application est déjà présente

---

Date de la build : 26 octobre 2026
Cible : Android 7.0+ (API 24+), `targetSdk` 34
Architecture supportée : arm64-v8a (vérifiée par `aapt2 dump badging` → `native-code: 'arm64-v8a'`)
