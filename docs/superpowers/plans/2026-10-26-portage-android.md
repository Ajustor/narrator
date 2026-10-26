# Portage Android — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Faire compiler et tourner l'app Dioxus existante (`crates/app_desktop`) sur Android, en réutilisant la quasi-totalité du code de l'étape 3, avec un téléchargement du modèle au premier lancement.

**Architecture:** Un seul crate `app_desktop`, sélection de plateforme par deux features Cargo mutuellement exclusives (`plateforme-desktop` / `plateforme-android`) qui choisissent la feature Dioxus correspondante (`desktop` / `mobile`) — les deux partagent déjà le même moteur webview (`dioxus-desktop`), donc vues/styles/protocole restent inchangés. Ajout ciblé : un nouvel état `Vue::Telechargement(f32)` affiché par le thread de jeu quand le fichier `.gguf` est absent, avant le chargement du modèle.

**Tech Stack:** Rust, Dioxus 0.7 (`desktop`/`mobile` features), `cargo-ndk` pour la compilation croisée Android, `ureq` (HTTP synchrone) pour le téléchargement, `jni`/`ndk-context` pour obtenir le répertoire de stockage privé de l'app sur Android.

**Spec:** `docs/superpowers/specs/2026-10-26-portage-android-design.md`

## Global Constraints

- La dépendance `dioxus` du crate `app_desktop` passe à `default-features = false, features = ["document"]` ; `desktop`/`mobile` sont ajoutées uniquement via les features de package `plateforme-desktop`/`plateforme-android` (`default = ["plateforme-desktop"]`).
- Aucune modification à `crates/domain`, `crates/application`, `crates/infra`, ou `crates/app` (console) — ce portage touche uniquement `crates/app_desktop`.
- URL et nom de fichier du modèle, à reprendre EXACTEMENT de `scripts/jouer.sh` : `https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf`.
- `cargo test --workspace` doit passer après chaque tâche.
- Le comportement desktop existant (variable d'environnement `NARRATEUR_MODELE`, pas de téléchargement en pratique) ne doit subir aucune régression.
- Aucune vérification automatisée sur émulateur Android dans ce plan — la vérification finale sur appareil réel est la responsabilité de l'utilisateur, pas une tâche de ce plan.
- Deux points d'implémentation sont volontairement laissés en sketch à vérifier contre la vraie API/le vrai comportement de l'outil au moment de l'exécution (mécanisme d'entrée Android de Dioxus ; API pour le répertoire privé de stockage Android via `jni`/`ndk-context`) : le compilateur et la sortie de `dx`/`cargo ndk` font autorité, pas le code sketché ci-dessous — c'est le même principe que toutes les incertitudes Dioxus déjà rencontrées et résolues avec succès à l'étape 3 de ce projet.

---

### Task 1: Features Cargo + module de téléchargement (logique pure + téléchargement réseau)

**Files:**
- Modify: `crates/app_desktop/Cargo.toml`
- Create: `crates/app_desktop/src/telechargement.rs`
- Modify: `crates/app_desktop/src/main.rs:1-4` (ajouter `mod telechargement;`)

**Interfaces:**
- Produces: `telechargement::MODELE_URL: &str`, `telechargement::faut_il_telecharger(chemin: &std::path::Path) -> bool`, `telechargement::telecharger_modele(chemin_destination: &std::path::Path, sur_progression: &mut dyn FnMut(f32)) -> Result<(), telechargement::ErreurTelechargement>`, `telechargement::ErreurTelechargement` (implémente `std::fmt::Debug`).

- [ ] **Step 1: Modifier `Cargo.toml`**

Remplacer le contenu actuel par :

```toml
[package]
name = "app_desktop"
version = "0.1.0"
edition = "2021"

[features]
default = ["plateforme-desktop"]
plateforme-desktop = ["dioxus/desktop"]
plateforme-android = ["dioxus/mobile", "dep:ndk-context", "dep:jni"]

[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
infra = { path = "../infra" }
dioxus = { version = "0.7", default-features = false, features = ["document"] }
futures-channel = "0.3"
futures-util = "0.3"
ureq = { version = "2", default-features = false, features = ["rustls"] }
ndk-context = { version = "0.1", optional = true }
jni = { version = "0.21", optional = true }
```

Note pour l'implémenteur : si le nom de feature `"rustls"` d'`ureq` a changé dans la version résolue (vérifier avec `cargo doc -p ureq --open` ou la page docs.rs de la version résolue), utiliser le nom correct — l'objectif est d'éviter de dépendre d'OpenSSL natif pour la compilation croisée Android, pas ce nom précis.

- [ ] **Step 2: Écrire les tests (qui doivent échouer à la compilation, le module n'existe pas encore)**

Créer `crates/app_desktop/src/telechargement.rs` avec seulement ce contenu pour commencer :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faut_il_telecharger_vrai_si_fichier_absent() {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_test_absent_{}.gguf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&chemin);
        assert!(faut_il_telecharger(&chemin));
    }

    #[test]
    fn faut_il_telecharger_faux_si_fichier_present() {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_test_present_{}.gguf",
            std::process::id()
        ));
        std::fs::write(&chemin, b"contenu factice").expect("ecriture test");
        assert!(!faut_il_telecharger(&chemin));
        let _ = std::fs::remove_file(&chemin);
    }
}
```

- [ ] **Step 3: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test -p app_desktop telechargement`
Expected: FAIL avec une erreur de compilation (`faut_il_telecharger` n'existe pas).

- [ ] **Step 4: Implémenter le module au complet**

Ajouter avant le `#[cfg(test)]` dans `crates/app_desktop/src/telechargement.rs` :

```rust
use std::io::{Read, Write};
use std::path::Path;

pub const MODELE_URL: &str =
    "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf";

#[derive(Debug)]
pub enum ErreurTelechargement {
    Reseau(String),
    Ecriture(String),
}

/// Décision pure : le fichier modèle est-il déjà présent à l'emplacement
/// attendu ? Sur desktop, ce chemin pointe déjà vers un fichier existant
/// (téléchargé par `scripts/jouer.sh` avant le lancement) — cette fonction
/// retourne donc `false` et le téléchargement ne se déclenche jamais en
/// usage normal sur desktop.
pub fn faut_il_telecharger(chemin_modele: &Path) -> bool {
    !chemin_modele.exists()
}

/// Télécharge le modèle vers `chemin_destination`, en passant la
/// progression (0.0 à 1.0) à `sur_progression` au fil du flux. Écrit dans
/// un fichier `.tmp` puis renomme de façon atomique vers la destination
/// finale une fois le téléchargement terminé — même principe que
/// `scripts/jouer.sh`, pour ne jamais laisser un fichier partiel au nom
/// final si le téléchargement est interrompu.
pub fn telecharger_modele(
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    let reponse = ureq::get(MODELE_URL)
        .call()
        .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;

    let taille_totale: u64 = reponse
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let chemin_temporaire = chemin_destination.with_extension("tmp");
    if let Some(parent) = chemin_temporaire.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
    }
    let mut fichier = std::fs::File::create(&chemin_temporaire)
        .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;

    let mut lecteur = reponse.into_reader();
    let mut tampon = [0u8; 65536];
    let mut telecharge: u64 = 0;
    loop {
        let n = lecteur
            .read(&mut tampon)
            .map_err(|e| ErreurTelechargement::Reseau(e.to_string()))?;
        if n == 0 {
            break;
        }
        fichier
            .write_all(&tampon[..n])
            .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
        telecharge += n as u64;
        if taille_totale > 0 {
            sur_progression(telecharge as f32 / taille_totale as f32);
        }
    }

    std::fs::rename(&chemin_temporaire, chemin_destination)
        .map_err(|e| ErreurTelechargement::Ecriture(e.to_string()))?;
    Ok(())
}
```

Ajouter `mod telechargement;` en tête de `crates/app_desktop/src/main.rs`, à côté des autres `mod`.

- [ ] **Step 5: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test -p app_desktop telechargement`
Expected: PASS (2 tests).

- [ ] **Step 6: Vérifier que le reste du crate compile toujours**

Run: `cargo build -p app_desktop`
Expected: compile sans erreur (features par défaut = `plateforme-desktop`).

- [ ] **Step 7: Commit**

```bash
git add crates/app_desktop/Cargo.toml crates/app_desktop/src/telechargement.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add Cargo features for platform selection and a model-download module"
```

---

### Task 2: Nouvel état `Vue::Telechargement` — protocole, vue, style, câblage UI

**Files:**
- Modify: `crates/app_desktop/src/protocole.rs`
- Create: `crates/app_desktop/src/vues/telechargement.rs`
- Modify: `crates/app_desktop/src/vues/mod.rs`
- Modify: `crates/app_desktop/src/styles.rs`
- Modify: `crates/app_desktop/src/main.rs`

**Interfaces:**
- Consumes: rien de nouveau depuis Task 1 (cette tâche ne branche pas encore `telechargement::telecharger_modele` — ça arrive à la Task 3).
- Produces: `protocole::Vue::Telechargement(f32)`, `protocole::MessageVersUi::ProgressionTelechargement(f32)`, composant `vues::telechargement::Telechargement { progression: f32 }`.

- [ ] **Step 1: Ajouter les nouvelles variantes au protocole**

Dans `crates/app_desktop/src/protocole.rs`, ajouter une variante à `MessageVersUi` :

```rust
#[derive(Debug, Clone)]
pub enum MessageVersUi {
    EtatInitial(Option<EtatPartie>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, combat_termine: bool },
    TourEchoue(String),
    PartieReinitialisee,
    ProgressionTelechargement(f32),
}
```

et une variante à `Vue` :

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
    Telechargement(f32),
}
```

- [ ] **Step 2: Créer la nouvelle vue**

Créer `crates/app_desktop/src/vues/telechargement.rs` :

```rust
use dioxus::prelude::*;

#[component]
pub fn Telechargement(progression: f32) -> Element {
    let pourcentage = (progression.clamp(0.0, 1.0) * 100.0).round() as i32;
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Téléchargement du modèle" }
                p { "Premier lancement : téléchargement du modèle de narration ({pourcentage} %)." }
                div { class: "barre-progression",
                    div {
                        class: "barre-progression-remplissage",
                        style: "width: {pourcentage}%;",
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Déclarer le module**

Dans `crates/app_desktop/src/vues/mod.rs`, ajouter une ligne pour obtenir :

```rust
pub mod creation;
pub mod erreur_chargement;
pub mod fin_de_partie;
pub mod jeu;
pub mod telechargement;
```

- [ ] **Step 4: Ajouter le style de la barre de progression**

Dans `crates/app_desktop/src/styles.rs`, ajouter avant le `"#;` final :

```css
.barre-progression {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    height: 1rem;
    overflow: hidden;
}

.barre-progression-remplissage {
    background: var(--accent);
    height: 100%;
    transition: width 0.2s ease;
}
```

- [ ] **Step 5: Câbler la vue dans `main.rs`**

Dans `crates/app_desktop/src/main.rs`, ajouter l'import :

```rust
use vues::telechargement::Telechargement;
```

Ajouter un bras de match dans la boucle `use_future` (juste avant le bras `MessageVersUi::PartieReinitialisee`) :

```rust
MessageVersUi::ProgressionTelechargement(progression) => {
    vue.set(Vue::Telechargement(progression));
}
```

Ajouter un bras dans le `match vue()` qui construit `vue_active` (ordre indifférent, par exemple juste avant le bras `Vue::FinDePartie`) :

```rust
Vue::Telechargement(progression) => rsx! { Telechargement { progression } },
```

- [ ] **Step 6: Vérifier la compilation et les tests**

Run: `cargo test -p app_desktop`
Expected: PASS (tous les tests existants + ceux de Task 1, aucune régression).

Run: `cargo build -p app_desktop`
Expected: compile sans erreur.

- [ ] **Step 7: Commit**

```bash
git add crates/app_desktop/src/protocole.rs crates/app_desktop/src/vues/telechargement.rs crates/app_desktop/src/vues/mod.rs crates/app_desktop/src/styles.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add a Telechargement view and wire its progress message"
```

---

### Task 3: Branchement réel du téléchargement dans le thread de jeu

**Files:**
- Modify: `crates/app_desktop/src/worker.rs`
- Modify: `crates/app_desktop/src/main.rs`

**Interfaces:**
- Consumes: `telechargement::faut_il_telecharger`, `telechargement::telecharger_modele`, `telechargement::ErreurTelechargement` (Task 1) ; `MessageVersUi::ProgressionTelechargement` (Task 2).
- Produces: `worker::chemin_modele_par_defaut() -> std::path::PathBuf` (résolution du chemin du modèle selon la plateforme, appelée depuis `main.rs` à la place de la lecture directe de `NARRATEUR_MODELE` quand la feature `plateforme-android` est active).

- [ ] **Step 1: Ajouter la résolution de chemin spécifique à la plateforme dans `worker.rs`**

Ajouter en haut de `crates/app_desktop/src/worker.rs`, après les `use` existants :

```rust
/// Chemin du fichier modèle selon la plateforme.
///
/// Desktop : inchangé, lu depuis la variable d'environnement
/// `NARRATEUR_MODELE` — ce chemin pointe déjà vers un fichier existant
/// (téléchargé par `scripts/jouer.sh`), donc `telechargement::faut_il_telecharger`
/// retournera toujours `false` en usage normal sur desktop.
///
/// Android : pas de variable d'environnement équivalente. Le modèle est
/// téléchargé dans le répertoire de stockage privé de l'app, obtenu via
/// JNI (`Activity.getFilesDir()`) à travers les crates `ndk-context`/`jni` —
/// c'est le mécanisme standard pour ce besoin dans l'écosystème Rust/Android,
/// mais il est à vérifier contre le vrai comportement du contexte Android
/// que `dioxus`/`tao` initialisent au démarrage de l'app : si
/// `ndk_context::android_context()` panique ou si l'API `jni` résolue
/// diffère de celle utilisée ci-dessous, adapter ce code en conséquence
/// plutôt que forcer cette signature — le compilateur et le comportement
/// réel sur appareil font autorité.
#[cfg(feature = "plateforme-desktop")]
pub fn chemin_modele_par_defaut() -> std::path::PathBuf {
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        std::process::exit(1);
    });
    std::path::PathBuf::from(chemin_modele)
}

#[cfg(feature = "plateforme-android")]
pub fn chemin_modele_par_defaut() -> std::path::PathBuf {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .expect("JavaVM Android valide fournie par le contexte natif");
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
    let mut env = vm
        .attach_current_thread()
        .expect("attachement du thread courant à la JVM Android");
    let fichier = env
        .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])
        .expect("appel de Activity.getFilesDir()")
        .l()
        .expect("getFilesDir() retourne un java.io.File");
    let chemin_jstring = env
        .call_method(&fichier, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .expect("appel de File.getAbsolutePath()")
        .l()
        .expect("getAbsolutePath() retourne une java.lang.String");
    let chemin_java_str = env
        .get_string(&jni::objects::JString::from(chemin_jstring))
        .expect("conversion JString -> JavaStr");
    let repertoire = std::path::PathBuf::from(chemin_java_str.to_str().expect("UTF-8 valide"));
    repertoire.join("Qwen2.5-3B-Instruct-Q4_K_M.gguf")
}
```

- [ ] **Step 2: Modifier `lancer()` pour télécharger avant de charger le modèle**

Dans `crates/app_desktop/src/worker.rs`, au début de la fonction `lancer`, avant la ligne `let mut narrateur = match infra::LlamaNarrateur::charger(&chemin_modele) {`, insérer :

```rust
    if crate::telechargement::faut_il_telecharger(&chemin_modele) {
        let resultat = crate::telechargement::telecharger_modele(&chemin_modele, &mut |progression| {
            let _ = tx.unbounded_send(MessageVersUi::ProgressionTelechargement(progression));
        });
        if let Err(erreur) = resultat {
            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                "échec du téléchargement du modèle : {erreur:?}"
            )));
            return;
        }
    }
```

(`TourEchoue` est réutilisé volontairement : à ce stade du démarrage, `erreur_chargement_initiale` est encore `true` côté UI, donc ce message route déjà vers `Vue::ErreurChargement` — même mécanisme que l'échec de chargement du modèle qui suit juste après, pas besoin d'un nouveau type de message.)

- [ ] **Step 3: Faire utiliser `chemin_modele_par_defaut()` par `main.rs`**

Dans `crates/app_desktop/src/main.rs`, remplacer :

```rust
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        std::process::exit(1);
    });
```

par :

```rust
    let chemin_modele = worker::chemin_modele_par_defaut();
```

et retirer la ligne `let chemin_modele = std::path::PathBuf::from(chemin_modele);` dans la fermeture passée à `std::thread::spawn` juste en dessous (elle devient redondante — `chemin_modele` est déjà un `PathBuf`), en gardant le `.clone()` :

```rust
    std::thread::spawn({
        let chemin_modele = chemin_modele.clone();
        let chemin_sauvegarde = chemin_sauvegarde.clone();
        move || worker::lancer(rx_vers_worker, tx_vers_ui, chemin_modele, chemin_sauvegarde)
    });
```

- [ ] **Step 4: Vérifier la non-régression desktop**

Run: `cargo test -p app_desktop`
Expected: PASS (tous les tests existants, aucune régression — `chemin_modele_par_defaut()` sous `plateforme-desktop` reproduit exactement l'ancien comportement).

Run: `cargo build -p app_desktop`
Expected: compile sans erreur.

Ne PAS déclencher de vrai téléchargement du modèle réel (plusieurs Go) dans ce test — le modèle de développement existe déjà localement (`modeles/`, utilisé via `NARRATEUR_MODELE`), donc `faut_il_telecharger` retourne `false` et ce chemin ne s'exécute jamais pendant le développement normal sur desktop. La vérification bout-en-bout d'un vrai téléchargement se fait manuellement une seule fois, sur un appareil Android réel, par l'utilisateur (voir Task 5).

- [ ] **Step 5: Vérifier manuellement qu'une partie desktop se lance toujours normalement**

Run (avec `NARRATEUR_MODELE` déjà positionné comme avant ce portage, et en utilisant la technique déjà établie pour ce projet pour les vérifications GUI en sandbox si nécessaire) : `cargo run -p app_desktop` puis créer un personnage.
Expected: l'écran de création s'affiche normalement (pas de passage par `Vue::Telechargement`), identique au comportement d'avant cette tâche.

- [ ] **Step 6: Commit**

```bash
git add crates/app_desktop/src/worker.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): download the model on first launch when it's missing"
```

---

### Task 4: Compilation croisée Android du crate complet + scaffolding `dx`

**Files:**
- Possibly create (selon ce que `dx` génère réellement — voir Step 2) : fichiers de configuration Android sous `crates/app_desktop/` (`Dioxus.toml` et/ou un dossier `android/`).
- Modify (si nécessaire, selon ce que `dx build` réclame) : `crates/app_desktop/Cargo.toml`.

**Interfaces:**
- Consumes: le crate `app_desktop` complet tel qu'il existe après Task 3.
- Produces: confirmation que `app_desktop` (pas seulement `infra`, comme lors du spike) compile pour `aarch64-linux-android`, et un scaffolding Android fonctionnel pour `dx`.

- [ ] **Step 1: Préparer l'environnement de compilation croisée**

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export ANDROID_NDK_HOME=/home/alexandre/Android/Sdk/ndk/29.0.14206865
dx doctor
```
Expected: `dx doctor` ne rapporte plus `java_home` manquant ; tout est vert pour Android (SDK, NDK, cibles Rust — déjà confirmés présents lors du spike de faisabilité).

- [ ] **Step 2: Compiler le crate complet pour Android via `cargo-ndk`**

```bash
cargo ndk -t arm64-v8a -P 24 build -p app_desktop --no-default-features --features plateforme-android,document
```

Expected: compile sans erreur et produit un artefact sous `target/aarch64-linux-android/debug/`. C'est la vraie vérification que le code de Task 1-3 (y compris le nouveau code `jni`/`ndk-context` de Task 3) compile réellement pour Android — contrairement au spike initial qui n'avait testé que `infra` seul.

**Si ça échoue** à cause de l'API `jni`/`ndk-context` sketchée en Task 3 (noms de méthodes différents dans la version résolue, signatures différentes, etc.) : corriger `chemin_modele_par_defaut()` sous `#[cfg(feature = "plateforme-android")]` pour qu'il compile réellement, en gardant son comportement voulu (retourner le chemin du fichier modèle dans le répertoire privé de l'app) — le message d'erreur du compilateur et la documentation de la version résolue de `jni` font autorité sur le sketch écrit en Task 3.

- [ ] **Step 3: Générer le scaffolding Android via `dx`**

Depuis `crates/app_desktop/` :

```bash
dx build --platform android --package app_desktop --no-default-features --features plateforme-android,document
```

**Si `dx` réclame un fichier de configuration manquant** (par exemple un `Dioxus.toml` avec un identifiant de package, un nom d'application, ou un niveau d'API minimum) : le créer avec les valeurs minimales demandées par le message d'erreur de `dx` (par exemple un identifiant de package `com.alexandregambier.narrateur` et le nom d'affichage "Narrateur").

- [ ] **Step 4: Décider quoi committer**

Inspecter ce que `dx` a produit (`git status` après le build). Pour chaque fichier/dossier nouvellement créé :
- S'il est entièrement régénérable à partir de `Cargo.toml` + `Dioxus.toml` + le code source (manifeste Android généré, structure Gradle générée) : NE PAS le committer — l'ajouter à `.gitignore` à la place, comme `target/`.
- S'il contient une configuration qui ne peut PAS être régénérée automatiquement et qui doit persister (le `Dioxus.toml` lui-même, des ressources/icônes ajoutées à la main) : le committer.

- [ ] **Step 5: Vérifier la non-régression desktop**

Run: `cargo test --workspace`
Expected: PASS (aucune régression sur les autres crates).

Run: `cargo build -p app_desktop`
Expected: compile toujours normalement avec les features par défaut (`plateforme-desktop`).

- [ ] **Step 6: Commit**

```bash
git add -A -- crates/app_desktop .gitignore
git commit -m "build(app_desktop): verify and wire up Android cross-compilation via dx and cargo-ndk"
```

(Adapter la liste de fichiers ajoutés au `git add` selon le résultat réel du Step 4 — ne committer que ce qui a été décidé comme non-régénérable.)

---

### Task 5: Vérification finale et instructions pour l'appareil réel

**Files:**
- Create: `docs/superpowers/plans/2026-10-26-portage-android-instructions-appareil.md` (instructions pour l'utilisateur, pas du code)

**Interfaces:**
- Consumes: le résultat de build produit par Task 4.
- Produces: rien de consommé par du code ultérieur — c'est la tâche de clôture.

- [ ] **Step 1: Suite de tests complète**

Run: `cargo test --workspace`
Expected: PASS, 0 échec.

- [ ] **Step 2: Construire l'APK de debug**

```bash
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export ANDROID_NDK_HOME=/home/alexandre/Android/Sdk/ndk/29.0.14206865
dx build --platform android --package app_desktop --no-default-features --features plateforme-android,document
```

Expected: production d'un `.apk` installable (chercher sous `target/dx/app_desktop/` ou l'emplacement indiqué par la sortie de `dx` — noter le chemin exact trouvé, il va dans les instructions du Step 3).

- [ ] **Step 3: Écrire les instructions d'installation sur appareil réel**

Créer `docs/superpowers/plans/2026-10-26-portage-android-instructions-appareil.md` avec :
- Le chemin exact de l'APK produit par le Step 2.
- La commande pour l'installer sur un appareil connecté en USB avec le débogage activé : `adb install -r <chemin-vers-l-apk>`.
- Un rappel que le premier lancement va télécharger ~2 Go (le modèle Qwen2.5-3B-Instruct-Q4_K_M.gguf) et nécessite donc une connexion réseau active et suffisamment d'espace de stockage libre sur l'appareil.
- Un rappel que c'est à l'utilisateur de vérifier visuellement le rendu et le bon fonctionnement sur son appareil réel (décision actée dans le spec) — lister les points à vérifier : écran de téléchargement avec barre de progression visible, transition vers l'écran de création après téléchargement, puis le reste du jeu identique au desktop.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/2026-10-26-portage-android-instructions-appareil.md
git commit -m "docs: add real-device installation instructions for the Android build"
```
