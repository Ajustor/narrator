# Design — Portage Android

Date : 2026-10-26
Auteur : Alexandre Gambier (avec Claude)
Contexte : étape 4 de la feuille de route ("portage Android"). L'étape 3 (App Dioxus desktop, `crates/app_desktop`) est terminée et fusionnée dans `master`, thème/mise en page compris. Un spike de faisabilité préalable (voir ci-dessous) a confirmé que la dépendance la plus risquée du projet, `llama-cpp-2` (qui vendorise et compile le cœur C++ de llama.cpp via `cmake`), compile sans accroc pour une cible Android (`aarch64-linux-android`) dans cet environnement.

## Résultat du spike de faisabilité (déjà effectué, pas à refaire)

```bash
cargo install cargo-ndk
export ANDROID_NDK_HOME=/home/alexandre/Android/Sdk/ndk/29.0.14206865
cargo ndk -t arm64-v8a -P 24 build -p infra
```

Compile proprement en ~1 min (`llama-cpp-sys-2`, `llama-cpp-2`, `infra` inclus), produit un `libinfra.rlib` réel sous `target/aarch64-linux-android/debug/`. L'environnement est par ailleurs déjà largement préparé pour le développement Android : SDK + NDK 29 installés, les quatre cibles Rust Android (`aarch64-linux-android`, `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android`) déjà ajoutées via `rustup`, CLI Dioxus (`dx` 0.7.9) déjà installée, JDK 21 présent (`/usr/lib/jvm/java-21-openjdk-amd64`, juste `JAVA_HOME` à exporter). `dx doctor` confirme tout au vert côté Android, à l'exception de `java_home` (variable d'environnement non exportée, pas un outil manquant).

Autre constat utile du spike : la feature `mobile` de Dioxus dépend de `dioxus-desktop` — **la même brique** que la feature `desktop` déjà utilisée par `crates/app_desktop`. Le rendu Android passe donc par le même moteur webview que le desktop (déjà vérifié visuellement à l'étape 3, thème sombre compris) — pas un moteur de rendu différent à reconcevoir.

## Objectif

Faire tourner l'app Dioxus existante sur Android, en réutilisant la quasi-totalité du code déjà écrit et vérifié à l'étape 3 (vues, styles, logique du thread de jeu, protocole de messages), avec deux ajouts spécifiques à Android : le point d'entrée de la plateforme, et le téléchargement du modèle au premier lancement (explicitement anticipé et mis hors scope de l'étape LLM-narrateur à l'époque : « Téléchargement automatique du modèle au premier lancement — fonctionnalité d'UX réservée à l'étape mobile/Android »).

## Hors scope

- iOS (XCode absent de cet environnement, aucune cible Rust iOS installée — pas dans la feuille de route actuelle de toute façon).
- Modèle allégé spécifique au mobile — le même Qwen2.5-3B-Instruct Q4_K_M que le desktop est utilisé, décision confirmée.
- Vérification automatisée sur émulateur Android — explicitement laissée de côté ; la vérification finale se fait sur un appareil réel, par l'utilisateur lui-même.
- Toute modification à `domain`/`application`/`infra`/`crates/app` (console) — `infra` est déjà prouvé compiler pour Android sans changement par le spike.
- Les deux lacunes déjà connues et différées à l'étape 3 (absence de retour à l'exploration après un combat, rendant le panneau PNJ inatteignable en jeu réel ; voir la mémoire du projet) — ni aggravées ni résolues par ce sous-projet, qui porte uniquement sur la plateforme.

## Décision d'architecture clé : un seul crate, sélection de plateforme par feature Cargo

`crates/app_desktop` garde son nom (le renommer serait un coût cosmétique sans bénéfice réel) et devient multi-plateforme via deux features qui s'excluent :

```toml
[features]
default = ["plateforme-desktop"]
plateforme-desktop = ["dioxus/desktop"]
plateforme-android = ["dioxus/mobile"]

[dependencies]
dioxus = { version = "0.7", default-features = false, features = ["document"] }
```

(`document` reste toujours actif, indépendamment de la plateforme — `document::Style`/`document::eval`, déjà utilisés par le thème desktop, s'appliquent identiquement sur Android puisque c'est le même moteur webview sous-jacent.)

Toutes les vues (`vues/*.rs`), `styles.rs`, `protocole.rs`, et l'essentiel de `worker.rs` restent partagés à 100 %, sans branche conditionnelle. `main.rs` garde un unique `fn main()` — le mécanisme exact par lequel Android route vers ce point d'entrée (très probablement une macro fournie par `tao` — `tao-macros` existe dans les dépendances transitives de `dioxus-desktop` — plutôt qu'un `android_main` à écrire à la main) est à vérifier contre la vraie API au moment de l'implémentation, dans le même esprit que toutes les incertitudes Dioxus précédentes de ce projet : le compilateur fait autorité, pas une supposition écrite à l'avance.

## Téléchargement du modèle au premier lancement (nouveau, Android uniquement en pratique)

Un nouvel état `Vue::Telechargement(f32)` (progression 0.0–1.0), affiché avant `Vue::Creation` quand le fichier modèle est absent du stockage de l'app. Le thread de jeu (`worker::lancer`) télécharge le `.gguf` avant d'appeler `LlamaNarrateur::charger`, en envoyant `MessageVersUi::ProgressionTelechargement(f32)` au fil du téléchargement.

Réutilise exactement l'URL et le nom de fichier déjà utilisés par `scripts/jouer.sh` :
```
https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf
```

Ce chemin ne se déclenche jamais sur desktop dans l'usage normal (le modèle y est supposé déjà présent, variable d'environnement `NARRATEUR_MODELE` pointant dessus comme aujourd'hui) — la logique « le fichier existe-t-il déjà » suffit à le rendre no-op sur desktop, pas besoin d'un `#[cfg(target_os = "android")]` séparé pour ça.

L'emplacement exact du stockage privé de l'app sur Android (répertoire accessible en écriture sans permission particulière, obtenu typiquement via les crates `android-activity`/`ndk-context` que `tao` utilise déjà en dépendance transitive) est à vérifier contre la vraie API disponible au moment de l'implémentation, même esprit que le point d'entrée ci-dessus.

Client HTTP : `ureq` (synchrone, cohérent avec le thread de jeu déjà synchrone — pas besoin d'un runtime async rien que pour ce téléchargement).

## Fichiers

- `crates/app_desktop/Cargo.toml` : features `plateforme-desktop`/`plateforme-android`, dépendance `ureq`.
- `crates/app_desktop/src/telechargement.rs` (nouveau) : logique de téléchargement (URL/chemin cible, suivi de progression).
- `crates/app_desktop/src/vues/telechargement.rs` (nouveau) : écran avec barre de progression.
- `crates/app_desktop/src/protocole.rs` : `Vue::Telechargement(f32)`, `MessageVersUi::ProgressionTelechargement(f32)`.
- `crates/app_desktop/src/worker.rs` : `lancer` télécharge le modèle si absent, avant `LlamaNarrateur::charger`.
- `crates/app_desktop/src/main.rs` : route le nouvel état vers la nouvelle vue.
- Scaffolding Android (`AndroidManifest.xml`, structure Gradle) : généré par `dx` lui-même au premier `dx build --platform android` — pas à écrire à la main, juste à inspecter/committer le résultat.

## Stratégie de test

- `cargo test -p app_desktop` (hôte, x86_64) : la décision « faut-il télécharger » (fichier déjà présent ou non) est testée comme logique pure dans `telechargement.rs`. Le téléchargement réseau lui-même n'est pas testé automatiquement — même principe que `LlamaNarrateur`, jamais couvert par des tests automatisés dans ce projet, vérifié manuellement à la place.
- `cargo build -p app_desktop --no-default-features --features plateforme-android,document --target aarch64-linux-android` doit compiler — c'est le prolongement direct du spike, maintenant sur le crate complet plutôt que sur `infra` seul.
- `dx build --platform android` doit produire un APK (commande exacte à confirmer contre la vraie CLI pendant l'implémentation).
- Aucune vérification automatisée sur émulateur dans ce sous-projet.

## Critères de complétion

- `cargo test --workspace` passe (inchangé pour tout ce qui n'est pas `app_desktop` ; les nouveaux tests de `telechargement.rs` passent).
- `cargo build -p app_desktop --no-default-features --features plateforme-android,document --target aarch64-linux-android` compile.
- `dx build --platform android` produit un APK installable.
- Le desktop reste fonctionnel sans régression (`cargo build -p app_desktop` avec les features par défaut compile et se comporte comme avant).
- Instructions claires fournies pour installer et lancer l'APK sur un appareil réel — la vérification visuelle/fonctionnelle finale sur appareil est faite par l'utilisateur, pas automatisée dans ce sous-projet.
