# App Dioxus desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Dioxus desktop GUI (`crates/app_desktop`, new) equivalent to the existing console app (`crates/app`, unchanged) — character creation, live-streamed narration, combat display, automatic save/load — plus a PNJ fiche viewer, without touching `domain`/`application`/`infra` or the console app.

**Architecture:** A dedicated `std::thread` owns the entire game loop (`EtatPartie`, `LlamaNarrateur`, `GenerateurDeOs`, `SauvegardeFichier`) for the app's whole lifetime — the same structure as the console's `main()`, with stdin/stdout replaced by channels. `std::sync::mpsc` carries player input into the worker (synchronous send from UI event handlers); `futures_channel::mpsc::unbounded` carries narration fragments and turn results back out, drained by a Dioxus `use_future` task that updates signals.

**Tech Stack:** Dioxus 0.7 (`features = ["desktop"]`), `futures-channel`, the existing `domain`/`application`/`infra` crates (path dependencies, unchanged).

**Spec:** `docs/superpowers/specs/2026-10-26-app-dioxus-desktop-design.md`

## Global Constraints

- New crate only: `crates/app_desktop`. `crates/app` (console), `domain`, `application`, `infra` are **not modified** by this plan.
- The LLM model never crosses a thread boundary after loading — it lives on one dedicated worker thread for the app's lifetime, exactly like the console's `main()`.
- `historique` on a `FichePnj` is for display only in the PNJ panel — nothing in this plan feeds it back into a prompt (that invariant belongs to `infra`, already shipped, untouched here).
- The Dioxus ecosystem moves fast: every code block below citing a hook name or macro (`use_signal`, `use_future`, `use_context_provider`, `rsx!`, `dioxus::launch`) is believed correct for Dioxus 0.7 as of this plan's writing, but **the compiler is the authority, not this text**. If `cargo build -p app_desktop` reports a different real signature or name for one of these, adapt to what the installed crate actually exposes (check `cargo doc -p dioxus --open` or the version pinned in `Cargo.lock`) rather than fighting the compiler — this mirrors how this project has handled unavoidable upstream-API drift before (e.g. the `llama-cpp-2` grammar API).
- No automated tests for Dioxus rendering itself (no precedent in this codebase for testing a UI's visual output) — verified manually per task. The worker's message-handling logic (`traiter_message`) is pure and must be unit-tested like any other `application`-style logic.

---

### Task 1: Scaffold `app_desktop` and the message/view protocol types

**Files:**
- Create: `crates/app_desktop/Cargo.toml`
- Create: `crates/app_desktop/src/main.rs` (minimal placeholder, replaced fully in Task 6)
- Create: `crates/app_desktop/src/protocole.rs`
- Modify: `Cargo.toml` (workspace root)

**Interfaces:**
- Produces: `MessageVersWorker`, `MessageVersUi`, `Vue` (all `pub`, in `crates/app_desktop/src/protocole.rs`) — consumed by every later task in this plan.

This task's only goal is derisking the new dependency and establishing the shared vocabulary everything else builds on. No business logic, no tests beyond "it compiles and runs."

- [ ] **Step 1: Register the new workspace member**

Open `Cargo.toml` at the repo root. Add `"crates/app_desktop"` to the `members` list:

```toml
[workspace]
resolver = "2"
members = [
    "crates/domain",
    "crates/application",
    "crates/infra",
    "crates/app",
    "crates/app_desktop",
]
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/app_desktop/Cargo.toml`:

```toml
[package]
name = "app_desktop"
version = "0.1.0"
edition = "2021"

[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
infra = { path = "../infra" }
dioxus = { version = "0.7", features = ["desktop"] }
futures-channel = "0.3"
futures-util = "0.3"
```

- [ ] **Step 3: Write a minimal placeholder `main.rs`**

Create `crates/app_desktop/src/main.rs`:

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div { "Narrateur — app_desktop (placeholder)" }
    }
}
```

- [ ] **Step 4: Verify it builds and runs**

Run: `cargo build -p app_desktop`

Expected: compiles cleanly. If the `dioxus` crate resolves to a version whose `launch`/`component`/`rsx!` surface differs from above, fix this placeholder to match what actually compiles — this step exists specifically to catch that early, before any real logic depends on it.

Optionally run `cargo run -p app_desktop` to confirm a window actually opens showing the placeholder text (this needs a graphical environment; if none is available in your execution context, compiling successfully is sufficient for this step — a later task's manual verification needs a real display).

- [ ] **Step 5: Add the protocol types**

Create `crates/app_desktop/src/protocole.rs`:

```rust
use application::usecases::jouer_tour::ResultatTour;
use domain::{EtatPartie, Vainqueur};

#[derive(Debug, Clone)]
pub enum MessageVersWorker {
    CreerPersonnage { nom: String, classe: String, valeurs: [u32; 6] },
    JouerTour(String),
    NouvellePartie,
}

#[derive(Debug, Clone)]
pub enum MessageVersUi {
    EtatInitial(Option<EtatPartie>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, combat_termine: bool },
    TourEchoue(String),
    PartieReinitialisee,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
}
```

Note: `MessageVersUi::TourTermine` carries `ResultatTour`'s three fields directly (not the struct itself) because `ResultatTour` has no `Clone`/`Debug` derive today and this plan does not modify `application` — destructuring here avoids needing to touch that struct's definition.

Add `mod protocole;` to the top of `crates/app_desktop/src/main.rs` (it will be unused until Task 2 — that's expected and fine for this task; an `#[allow(dead_code)]` on the `mod protocole;` line keeps the build warning-free until then).

- [ ] **Step 6: Run the build once more and commit**

Run: `cargo build -p app_desktop`

Expected: compiles cleanly (with the `#[allow(dead_code)]` suppressing the unused-module warning).

```bash
git add Cargo.toml crates/app_desktop
git commit -m "feat(app_desktop): scaffold the Dioxus desktop crate and message/view protocol types"
```

---

### Task 2: The worker — testable turn-handling core + thread entry point

**Files:**
- Create: `crates/app_desktop/src/worker.rs`
- Modify: `crates/app_desktop/src/main.rs` (add `mod worker;`)

**Interfaces:**
- Consumes: `MessageVersWorker`/`MessageVersUi` (Task 1); `application::usecases::jouer_tour::JouerTour`, `application::usecases::creer_personnage::creer_personnage`, `application::usecases::commencer_combat::commencer_combat`, `application::usecases::partie::{charger_partie, sauvegarder_partie}`; `application::ports::sauvegarde::{Sauvegarde, ErreurSauvegarde}`; `application::ports::generateur_de::GenerateurDe`; `application::ports::narrateur::Narrateur`; `domain::{Ennemi, De, EtatPartie, Vainqueur}`.
- Produces: `pub fn traiter_message(etat: &mut Option<EtatPartie>, message: MessageVersWorker, narrateur: &mut dyn Narrateur, de: &mut dyn GenerateurDe, sauvegarde: &dyn Sauvegarde, chemin_sauvegarde: &std::path::Path, sur_message_ui: &mut dyn FnMut(MessageVersUi))` (pure logic, no threads/channels — Task 6 is the only consumer of this crate's own UI, but this function is what Task 2's own tests exercise directly), and `pub fn lancer(rx: std::sync::mpsc::Receiver<MessageVersWorker>, tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>, chemin_modele: std::path::PathBuf, chemin_sauvegarde: std::path::PathBuf)` (the real thread entry point, consumed by Task 6's `main.rs`).

This is the only task in this plan with meaningful business logic (routing messages, mirroring `crates/app/src/main.rs`'s existing control flow) — it is also the only task with automated tests.

- [ ] **Step 1: Write the failing tests**

Create `crates/app_desktop/src/worker.rs` with just the imports and an empty test module to start:

```rust
use application::ports::generateur_de::GenerateurDe;
use application::ports::narrateur::Narrateur;
use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use application::usecases::commencer_combat::commencer_combat;
use application::usecases::creer_personnage::creer_personnage;
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, Ennemi, EtatPartie};
use std::path::Path;

use crate::protocole::{MessageVersUi, MessageVersWorker};

pub fn traiter_message(
    etat: &mut Option<EtatPartie>,
    message: MessageVersWorker,
    narrateur: &mut dyn Narrateur,
    de: &mut dyn GenerateurDe,
    sauvegarde: &dyn Sauvegarde,
    chemin_sauvegarde: &Path,
    sur_message_ui: &mut dyn FnMut(MessageVersUi),
) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::FauxNarrateur;
    use infra::SauvegardeFichier;
    use std::path::PathBuf;

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("app_desktop_test_{}_{}.json", std::process::id(), nom))
    }

    struct GenerateurDeFixe;

    impl GenerateurDe for GenerateurDeFixe {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            domain::JetDe::nouveau(1, modificateur)
        }
    }

    #[test]
    fn creer_personnage_envoie_partie_creee() {
        let chemin = chemin_temporaire("creer");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = None;
        let mut narrateur = FauxNarrateur;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::CreerPersonnage {
                nom: "Elowen".to_string(),
                classe: "Guerrier".to_string(),
                valeurs: [15, 14, 13, 12, 10, 8],
            },
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(etat.is_some());
        assert_eq!(etat.as_ref().unwrap().personnage.nom, "Elowen");
        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::PartieCreee(_))));

        let _ = std::fs::remove_file(chemin);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p app_desktop`

Expected: the crate compiles (the `todo!()` is valid Rust, it panics at runtime, not at compile time) but `cargo test -p app_desktop` FAILS — the test panics when it reaches the `todo!()`.

- [ ] **Step 3: Implement `traiter_message`**

Replace the `todo!()` body:

```rust
pub fn traiter_message(
    etat: &mut Option<EtatPartie>,
    message: MessageVersWorker,
    narrateur: &mut dyn Narrateur,
    de: &mut dyn GenerateurDe,
    sauvegarde: &dyn Sauvegarde,
    chemin_sauvegarde: &Path,
    sur_message_ui: &mut dyn FnMut(MessageVersUi),
) {
    match message {
        MessageVersWorker::CreerPersonnage { nom, classe, valeurs } => {
            let personnage = match creer_personnage(&nom, &classe, valeurs) {
                Ok(personnage) => personnage,
                Err(_) => return,
            };
            let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
            let combat = commencer_combat(&personnage, vec![gobelin], de);
            let nouvel_etat = EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: Some(combat),
                resumes: Vec::new(),
                pnj_connus: Vec::new(),
            };
            *etat = Some(nouvel_etat.clone());
            sur_message_ui(MessageVersUi::PartieCreee(nouvel_etat));
        }

        MessageVersWorker::JouerTour(saisie) => {
            let Some(etat_courant) = etat.as_mut() else { return };
            let resultat = JouerTour::executer(etat_courant, &saisie, narrateur, de, &mut |fragment: &str| {
                sur_message_ui(MessageVersUi::NarrationFragment(fragment.to_string()));
            });

            match resultat {
                Ok(resultat) => {
                    if resultat.combat_termine {
                        let _ = std::fs::remove_file(chemin_sauvegarde);
                    } else {
                        let _ = sauvegarder_partie(sauvegarde, etat_courant);
                    }
                    sur_message_ui(MessageVersUi::TourTermine {
                        etat: etat_courant.clone(),
                        resultat_narration: resultat.narration,
                        resume_mecanique: resultat.resume_mecanique,
                        combat_termine: resultat.combat_termine,
                    });
                }
                Err(erreur) => {
                    sur_message_ui(MessageVersUi::TourEchoue(format!("{:?}", erreur)));
                }
            }
        }

        MessageVersWorker::NouvellePartie => {
            let _ = std::fs::remove_file(chemin_sauvegarde);
            *etat = None;
            sur_message_ui(MessageVersUi::PartieReinitialisee);
        }
    }
}

pub fn charger_etat_initial(sauvegarde: &dyn Sauvegarde) -> Option<EtatPartie> {
    match charger_partie(sauvegarde) {
        Ok(etat) => Some(etat),
        Err(ErreurSauvegarde::FichierAbsent) => None,
        Err(_) => None,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p app_desktop`

Expected: PASS for `creer_personnage_envoie_partie_creee`.

- [ ] **Step 5: Add the remaining tests**

Add these tests to the same `mod tests` block:

```rust
    struct NarrateurTourReussi;

    impl Narrateur for NarrateurTourReussi {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok("Il ne se passe rien de notable.".to_string())
        }

        fn condenser_resumes(
            &mut self,
            _anciens: &[String],
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(
            &mut self,
            _narration: &str,
            _noms_connus: &[String],
        ) -> Result<Option<application::ports::narrateur::PnjDetecte>, application::ports::narrateur::ErreurNarrateur> {
            Ok(None)
        }
    }

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: domain::Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                domain::Caracteristiques {
                    force: 12,
                    dexterite: 14,
                    constitution: 13,
                    intelligence: 10,
                    sagesse: 10,
                    charisme: 8,
                },
                vec![],
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
        }
    }

    #[test]
    fn jouer_tour_hors_combat_envoie_fragments_puis_tour_termine() {
        let chemin = chemin_temporaire("tour_reussi");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourReussi;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { combat_termine: false, .. })));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurTourEnEchec;

    impl Narrateur for NarrateurTourEnEchec {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::Attaquer { cible: "Gobelin".into() })
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn condenser_resumes(
            &mut self,
            _anciens: &[String],
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(
            &mut self,
            _narration: &str,
            _noms_connus: &[String],
        ) -> Result<Option<application::ports::narrateur::PnjDetecte>, application::ports::narrateur::ErreurNarrateur> {
            Ok(None)
        }
    }

    #[test]
    fn attaquer_sans_combat_envoie_tour_echoue() {
        let chemin = chemin_temporaire("tour_echec");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourEnEchec;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'attaque".to_string()),
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourEchoue(_))));

        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn nouvelle_partie_reinitialise_l_etat_et_supprime_la_sauvegarde() {
        let chemin = chemin_temporaire("nouvelle_partie");
        std::fs::write(&chemin, "contenu factice").expect("écriture échouée");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = FauxNarrateur;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::NouvellePartie,
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(etat.is_none());
        assert!(!chemin.exists());
        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::PartieReinitialisee)));
    }
```

- [ ] **Step 6: Run all the tests to verify they pass**

Run: `cargo test -p app_desktop`

Expected: PASS — all 4 tests green.

- [ ] **Step 7: Add the real thread entry point `lancer`**

Still in `crates/app_desktop/src/worker.rs`, add (outside the test module):

```rust
pub fn lancer(
    rx: std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    chemin_modele: std::path::PathBuf,
    chemin_sauvegarde: std::path::PathBuf,
) {
    let mut narrateur = match infra::LlamaNarrateur::charger(&chemin_modele) {
        Ok(narrateur) => narrateur,
        Err(erreur) => {
            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                "échec du chargement du modèle : {erreur}"
            )));
            return;
        }
    };
    let mut de = infra::GenerateurDeOs;
    let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde.clone());

    let mut etat = charger_etat_initial(&sauvegarde);
    let _ = tx.unbounded_send(MessageVersUi::EtatInitial(etat.clone()));

    while let Ok(message) = rx.recv() {
        traiter_message(
            &mut etat,
            message,
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin_sauvegarde,
            &mut |msg| {
                let _ = tx.unbounded_send(msg);
            },
        );
    }
}
```

Note: this reuses `MessageVersUi::TourEchoue` to carry the model-load-failure message, rather than inventing a new variant — Task 6's `main.rs` is responsible for recognizing that a `TourEchoue` arriving *before any* `EtatInitial`/`PartieCreee` means model loading failed, and routing to `Vue::ErreurChargement` instead of showing it as an in-game turn error. (An alternative would be a dedicated `MessageVersUi::ErreurChargement(String)` variant — if you find the reuse confusing while implementing Task 6, adding that variant to `protocole.rs` instead is a reasonable, small deviation; note it in your report if you do.)

- [ ] **Step 8: Add `mod worker;` to `main.rs`**

Open `crates/app_desktop/src/main.rs` and add `mod worker;` below the existing `mod protocole;` line. `lancer` is unused until Task 6 wires it up — add `#[allow(dead_code)]` to the `pub fn lancer` line for now (remove it in Task 6 once it's actually called).

- [ ] **Step 9: Run the full test suite and commit**

Run: `cargo test -p app_desktop`

Expected: PASS, all tests green, no warnings besides the intentional `#[allow(dead_code)]`.

```bash
git add crates/app_desktop
git commit -m "feat(app_desktop): add the worker thread's turn-handling logic, with tests"
```

---

### Task 3: Creation view

**Files:**
- Create: `crates/app_desktop/src/vues/creation.rs`
- Modify: `crates/app_desktop/src/main.rs` (add `mod vues;`)
- Create: `crates/app_desktop/src/vues/mod.rs`

**Interfaces:**
- Consumes: `MessageVersWorker::CreerPersonnage` (Task 1), `application::usecases::creer_personnage::{CLASSES_VALIDES, VALEURS_STANDARD}`.
- Produces: `#[component] pub fn Creation(sur_creation: EventHandler<MessageVersWorker>) -> Element` — consumed by Task 6's root `App` component.

No automated test for this task (Dioxus rendering, verified manually in Task 6's end-to-end check) — this task's own "done" bar is that it compiles and its logic for tracking remaining caractéristiques values is correct by inspection.

- [ ] **Step 1: Create the views module**

Create `crates/app_desktop/src/vues/mod.rs`:

```rust
pub mod creation;
```

(Tasks 4 and 5 add `pub mod jeu;`, `pub mod fin_de_partie;`, `pub mod erreur_chargement;` to this same file — each of those tasks' first step.)

Add `mod vues;` to `crates/app_desktop/src/main.rs`.

- [ ] **Step 2: Implement the Creation component**

Create `crates/app_desktop/src/vues/creation.rs`:

```rust
use application::usecases::creer_personnage::{CLASSES_VALIDES, VALEURS_STANDARD};
use dioxus::prelude::*;

use crate::protocole::MessageVersWorker;

const NOMS_CARACTERISTIQUES: [&str; 6] =
    ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];

#[component]
pub fn Creation(sur_creation: EventHandler<MessageVersWorker>) -> Element {
    let mut nom = use_signal(String::new);
    let mut classe = use_signal(|| CLASSES_VALIDES[0].to_string());
    let mut valeurs = use_signal(|| [0u32; 6]);

    let valeurs_disponibles = move |index_a_exclure: usize| -> Vec<u32> {
        let deja_choisies: Vec<u32> = valeurs()
            .iter()
            .enumerate()
            .filter(|(i, v)| *i != index_a_exclure && **v != 0)
            .map(|(_, v)| *v)
            .collect();
        VALEURS_STANDARD
            .iter()
            .copied()
            .filter(|v| !deja_choisies.contains(v) || valeurs()[index_a_exclure] == *v)
            .collect()
    };

    let toutes_valeurs_choisies = move || valeurs().iter().all(|v| *v != 0);

    rsx! {
        div {
            h1 { "Création de personnage" }
            div {
                label { "Nom : " }
                input {
                    value: "{nom}",
                    oninput: move |evt| nom.set(evt.value()),
                }
            }
            div {
                label { "Classe : " }
                select {
                    onchange: move |evt| classe.set(evt.value()),
                    for c in CLASSES_VALIDES.iter() {
                        option { value: "{c}", "{c}" }
                    }
                }
            }
            for (index, label) in NOMS_CARACTERISTIQUES.iter().enumerate() {
                div {
                    label { "{label} : " }
                    select {
                        onchange: move |evt| {
                            let mut v = valeurs();
                            v[index] = evt.value().parse().unwrap_or(0);
                            valeurs.set(v);
                        },
                        option { value: "0", "—" }
                        for valeur in valeurs_disponibles(index) {
                            option { value: "{valeur}", "{valeur}" }
                        }
                    }
                }
            }
            button {
                disabled: !toutes_valeurs_choisies(),
                onclick: move |_| {
                    sur_creation.call(MessageVersWorker::CreerPersonnage {
                        nom: nom(),
                        classe: classe(),
                        valeurs: valeurs(),
                    });
                },
                "Commencer l'aventure"
            }
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app_desktop`

Expected: compiles — `Creation` is unused until Task 6 renders it, so an `#[allow(dead_code)]` on `pub fn Creation` (or on the `mod vues;` line in `main.rs`) keeps the build warning-free until then.

- [ ] **Step 4: Commit**

```bash
git add crates/app_desktop/src/vues crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add the character creation view"
```

---

### Task 4: Jeu view (narration, combat panel, input, PNJ panel)

**Files:**
- Create: `crates/app_desktop/src/vues/jeu.rs`
- Modify: `crates/app_desktop/src/vues/mod.rs`

**Interfaces:**
- Consumes: `MessageVersWorker::JouerTour` (Task 1), `domain::EtatPartie` (for combat/PV/PNJ display).
- Produces: `#[component] pub fn Jeu(etat: EtatPartie, journal: Vec<String>, narration_en_cours: String, tour_en_cours: bool, sur_action: EventHandler<MessageVersWorker>) -> Element` — consumed by Task 6's root `App` component, which owns the actual signals and passes their current values down as props (this component itself holds no state beyond the text-input field's own local value).

No automated test (same reasoning as Task 3).

- [ ] **Step 1: Add the module declaration**

Add `pub mod jeu;` to `crates/app_desktop/src/vues/mod.rs`.

- [ ] **Step 2: Implement the Jeu component**

Create `crates/app_desktop/src/vues/jeu.rs`:

```rust
use dioxus::prelude::*;
use domain::EtatPartie;

use crate::protocole::MessageVersWorker;

#[component]
pub fn Jeu(
    etat: EtatPartie,
    journal: Vec<String>,
    narration_en_cours: String,
    tour_en_cours: bool,
    sur_action: EventHandler<MessageVersWorker>,
) -> Element {
    let mut saisie = use_signal(String::new);
    let mut pnj_deplie: Signal<Option<String>> = use_signal(|| None);

    let envoyer = move || {
        if !saisie().trim().is_empty() && !tour_en_cours {
            sur_action.call(MessageVersWorker::JouerTour(saisie()));
            saisie.set(String::new());
        }
    };

    rsx! {
        div {
            header {
                span { "{etat.lieu_actuel}" }
                span { " — PV : {etat.personnage.pv_actuels}/{etat.personnage.pv_max}" }
            }

            if let Some(combat) = etat.combat_en_cours.as_ref() {
                if !combat.termine {
                    div {
                        h3 { "Combat" }
                        for ennemi in combat.ennemis.iter().filter(|e| !e.est_mort()) {
                            div { "{ennemi.nom} : {ennemi.pv_actuels}/{ennemi.pv_max} PV" }
                        }
                    }
                }
            }

            div {
                for ligne in journal.iter() {
                    p { "{ligne}" }
                }
                if !narration_en_cours.is_empty() {
                    p { "{narration_en_cours}" }
                }
            }

            div {
                input {
                    value: "{saisie}",
                    disabled: tour_en_cours,
                    oninput: move |evt| saisie.set(evt.value()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            envoyer();
                        }
                    },
                }
                button {
                    disabled: tour_en_cours,
                    onclick: move |_| envoyer(),
                    "Envoyer"
                }
            }

            aside {
                h3 { "PNJ rencontrés" }
                for pnj in etat.pnj_connus.iter() {
                    div {
                        onclick: {
                            let nom = pnj.nom.clone();
                            move |_| {
                                let actuel = pnj_deplie();
                                pnj_deplie.set(if actuel.as_deref() == Some(nom.as_str()) { None } else { Some(nom.clone()) });
                            }
                        },
                        strong { "{pnj.nom}" }
                        span { " — {pnj.description}" }
                        if pnj_deplie().as_deref() == Some(pnj.nom.as_str()) {
                            ul {
                                for entree in pnj.historique.iter() {
                                    li { "{entree}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app_desktop`

Expected: compiles (with the same `#[allow(dead_code)]` treatment as Task 3 until Task 6 renders it). If `evt.key() == Key::Enter` doesn't match the real `dioxus::events` API in the resolved version, check `cargo doc -p dioxus --open` for the actual keyboard-event accessor and adjust — the intent (submit on Enter) matters more than the exact accessor name.

- [ ] **Step 4: Commit**

```bash
git add crates/app_desktop/src/vues
git commit -m "feat(app_desktop): add the main game view (narration, combat panel, PNJ panel)"
```

---

### Task 5: FinDePartie and ErreurChargement views

**Files:**
- Create: `crates/app_desktop/src/vues/fin_de_partie.rs`
- Create: `crates/app_desktop/src/vues/erreur_chargement.rs`
- Modify: `crates/app_desktop/src/vues/mod.rs`

**Interfaces:**
- Consumes: `domain::Vainqueur`, `MessageVersWorker::NouvellePartie` (Task 1).
- Produces: `#[component] pub fn FinDePartie(vainqueur: Option<domain::Vainqueur>, sur_nouvelle_partie: EventHandler<()>) -> Element` and `#[component] pub fn ErreurChargement(message: String) -> Element` — both consumed by Task 6.

Both views are small and the same shape (a message plus, for one of them, a button) — handled together in one task.

- [ ] **Step 1: Add the module declarations**

Add `pub mod fin_de_partie;` and `pub mod erreur_chargement;` to `crates/app_desktop/src/vues/mod.rs`.

- [ ] **Step 2: Implement FinDePartie**

Create `crates/app_desktop/src/vues/fin_de_partie.rs`:

```rust
use dioxus::prelude::*;
use domain::Vainqueur;

#[component]
pub fn FinDePartie(vainqueur: Option<Vainqueur>, sur_nouvelle_partie: EventHandler<()>) -> Element {
    let message = match vainqueur {
        Some(Vainqueur::Joueur) => "Victoire ! Le gobelin est vaincu.",
        Some(Vainqueur::Ennemis) => "Tu es tombé au combat...",
        Some(Vainqueur::Fuite) => "Tu as fui le combat.",
        None => "La partie est terminée.",
    };

    rsx! {
        div {
            h1 { "{message}" }
            button {
                onclick: move |_| sur_nouvelle_partie.call(()),
                "Nouvelle partie"
            }
        }
    }
}
```

- [ ] **Step 3: Implement ErreurChargement**

Create `crates/app_desktop/src/vues/erreur_chargement.rs`:

```rust
use dioxus::prelude::*;

#[component]
pub fn ErreurChargement(message: String) -> Element {
    rsx! {
        div {
            h1 { "Échec du chargement du modèle" }
            p { "{message}" }
            p { "Vérifie que la variable d'environnement NARRATEUR_MODELE pointe vers un fichier .gguf valide, puis relance l'application." }
        }
    }
}
```

- [ ] **Step 4: Verify it compiles and commit**

Run: `cargo build -p app_desktop`

Expected: compiles (both views unused until Task 6, same `#[allow(dead_code)]` treatment).

```bash
git add crates/app_desktop/src/vues
git commit -m "feat(app_desktop): add the end-of-game and model-load-error views"
```

---

### Task 6: Wire everything together in `main.rs`, end to end

**Files:**
- Modify: `crates/app_desktop/src/main.rs` (full rewrite of the placeholder)

**Interfaces:**
- Consumes: everything from Tasks 1-5 — `worker::lancer`, `protocole::{MessageVersWorker, MessageVersUi, Vue}`, `vues::{creation::Creation, jeu::Jeu, fin_de_partie::FinDePartie, erreur_chargement::ErreurChargement}`.
- Produces: the finished `app_desktop` binary. Nothing downstream of this task.

This task has no automated test of its own (it's pure wiring) — its correctness is established by the manual end-to-end verification in Step 3.

- [ ] **Step 1: Replace `main.rs` with the full wiring**

Replace the entire contents of `crates/app_desktop/src/main.rs`:

```rust
mod protocole;
mod vues;
mod worker;

use dioxus::prelude::*;
use futures_util::StreamExt;
use protocole::{MessageVersUi, MessageVersWorker, Vue};
use vues::creation::Creation;
use vues::erreur_chargement::ErreurChargement;
use vues::fin_de_partie::FinDePartie;
use vues::jeu::Jeu;

fn main() {
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        std::process::exit(1);
    });
    let chemin_sauvegarde = std::path::PathBuf::from("partie.json");

    let (tx_vers_worker, rx_vers_worker) = std::sync::mpsc::channel::<MessageVersWorker>();
    let (tx_vers_ui, rx_vers_ui) = futures_channel::mpsc::unbounded::<MessageVersUi>();

    std::thread::spawn({
        let chemin_modele = std::path::PathBuf::from(chemin_modele);
        let chemin_sauvegarde = chemin_sauvegarde.clone();
        move || worker::lancer(rx_vers_worker, tx_vers_ui, chemin_modele, chemin_sauvegarde)
    });

    // `UnboundedReceiver<MessageVersUi>` has a single owner and is not
    // `Clone` — it can't be handed to the root component as an ordinary
    // prop or context value the way `tx_vers_worker` (whose `Sender` type
    // IS `Clone`) can. `Rc<RefCell<Option<_>>>` is the standard one-time
    // handoff for this: cloning the `Rc` is cheap and shares the same
    // cell, and `.take()` inside `App`'s `use_future` (which Dioxus only
    // ever runs once, on first render) extracts the real receiver exactly
    // once. `Rc`/`RefCell` (not `Arc`/`Mutex`) are correct here — this
    // value never leaves the Dioxus UI thread.
    let rx_partage = std::rc::Rc::new(std::cell::RefCell::new(Some(rx_vers_ui)));

    dioxus::LaunchBuilder::desktop()
        .with_context(tx_vers_worker)
        .with_context(rx_partage)
        .launch(App);
}

type RxPartage = std::rc::Rc<std::cell::RefCell<Option<futures_channel::mpsc::UnboundedReceiver<MessageVersUi>>>>;

#[component]
fn App() -> Element {
    let tx = use_context::<std::sync::mpsc::Sender<MessageVersWorker>>();
    let rx_partage = use_context::<RxPartage>();
    let mut vue = use_signal(|| Vue::Creation);
    let mut etat = use_signal(|| None::<domain::EtatPartie>);
    let mut journal: Signal<Vec<String>> = use_signal(Vec::new);
    let mut narration_en_cours = use_signal(String::new);
    let mut tour_en_cours = use_signal(|| false);
    let mut erreur_chargement_initiale = use_signal(|| true);

    use_future(move || {
        let rx_partage = rx_partage.clone();
        async move {
        let mut rx = rx_partage
            .borrow_mut()
            .take()
            .expect("le récepteur ne doit être pris qu'une seule fois, par ce use_future");
        while let Some(message) = rx.next().await {
            match message {
                MessageVersUi::EtatInitial(Some(e)) => {
                    etat.set(Some(e));
                    vue.set(Vue::Jeu);
                    erreur_chargement_initiale.set(false);
                }
                MessageVersUi::EtatInitial(None) => {
                    vue.set(Vue::Creation);
                    erreur_chargement_initiale.set(false);
                }
                MessageVersUi::PartieCreee(e) => {
                    etat.set(Some(e));
                    vue.set(Vue::Jeu);
                }
                MessageVersUi::NarrationFragment(fragment) => {
                    narration_en_cours.write().push_str(&fragment);
                }
                MessageVersUi::TourTermine { etat: nouvel_etat, resultat_narration, combat_termine, .. } => {
                    journal.write().push(resultat_narration);
                    narration_en_cours.set(String::new());
                    tour_en_cours.set(false);
                    let vainqueur = nouvel_etat.combat_en_cours.as_ref().and_then(|c| c.vainqueur);
                    etat.set(Some(nouvel_etat));
                    if combat_termine {
                        vue.set(Vue::FinDePartie(vainqueur));
                    }
                }
                MessageVersUi::TourEchoue(message) => {
                    if erreur_chargement_initiale() {
                        vue.set(Vue::ErreurChargement(message));
                    } else {
                        journal.write().push(format!("⚠ {}", message));
                        tour_en_cours.set(false);
                    }
                }
                MessageVersUi::PartieReinitialisee => {
                    etat.set(None);
                    journal.set(Vec::new());
                    vue.set(Vue::Creation);
                }
            }
        }
        }
    });

    match vue() {
        Vue::ErreurChargement(message) => rsx! { ErreurChargement { message } },
        Vue::Creation => rsx! {
            Creation {
                sur_creation: move |msg| { let _ = tx.send(msg); }
            }
        },
        Vue::Jeu => {
            let Some(etat_actuel) = etat() else {
                return rsx! { div { "Chargement..." } };
            };
            rsx! {
                Jeu {
                    etat: etat_actuel,
                    journal: journal(),
                    narration_en_cours: narration_en_cours(),
                    tour_en_cours: tour_en_cours(),
                    sur_action: move |msg| {
                        tour_en_cours.set(true);
                        let _ = tx.send(msg);
                    },
                }
            }
        }
        Vue::FinDePartie(vainqueur) => rsx! {
            FinDePartie {
                vainqueur,
                sur_nouvelle_partie: move |_| { let _ = tx.send(MessageVersWorker::NouvellePartie); }
            }
        },
    }
}
```

Note: `dioxus::LaunchBuilder::desktop().with_context(...)` and `use_context::<T>()` are believed to be the correct Dioxus 0.7 pattern for providing the two non-prop values above (the `Sender<MessageVersWorker>` and the `RxPartage` one-time-handoff cell) to the root component without prop-drilling — per this plan's Global Constraints, verify against the actual resolved `dioxus` version and adapt if the real API differs. If `.with_context` on the builder turns out not to exist or not to work the way sketched here, the fallback is `use_context_provider` called inside `App` itself: have `main()` store `tx_vers_worker` and `rx_partage` somewhere `App` can reach without a hook (a `thread_local!` or a thinly-wrapped global, initialized once before `dioxus::launch`), and call `use_context_provider(|| ...)` for each at the top of `App` to re-expose them to the hook system for child components. The important invariants to preserve regardless of mechanism: (1) the UI→worker sender must be reachable from every view's event handlers without being threaded through every component's props by hand, and (2) the `UnboundedReceiver` must be moved out of its handoff cell exactly once, by the one `use_future` task that drains it — never cloned, never taken twice.

- [ ] **Step 2: Build and run the full workspace test suite**

Run: `cargo build -p app_desktop`

Expected: compiles. Fix any remaining API mismatches against the real `dioxus` version per the Global Constraints note.

Run: `cargo test --workspace`

Expected: PASS — nothing in this task changes any other crate's tests; `app_desktop`'s own tests (Task 2) still pass.

- [ ] **Step 3: Manual end-to-end verification**

Run, with a real model:

```bash
NARRATEUR_MODELE=/home/alexandre/dev/narrator/modeles/Qwen2.5-3B-Instruct-Q4_K_M.gguf cargo run -p app_desktop
```

Verify, in the running window:
- A new game (no `partie.json` present) shows the Creation view; creating a character with a name, a class, and all six caractéristiques assigned (each select only ever offering the still-available standard values) moves to the Jeu view with an initial Gobelin combat already active.
- Submitting an action shows the narration appearing progressively (the live typewriter effect), the input disabled while a turn is in flight, and re-enabled once it completes.
- During combat, the enemy's remaining PV update after an attack; the player's PV updates after taking damage.
- Finishing the combat (win, lose, or flee) shows the FinDePartie view with the right message; "Nouvelle partie" returns to Creation and `partie.json` no longer exists.
- A turn played outside combat that causes a PNJ to be detected (same technique as the fiches-PNJ sub-project's manual verification — mention an NPC by name across a few turns) makes that PNJ appear in the PNJ panel, clickable to show its `historique`.
- Quitting and relaunching with an in-progress (non-combat) save resumes directly into the Jeu view via `EtatInitial(Some(_))`, skipping Creation.
- Launching with `NARRATEUR_MODELE` pointing at a nonexistent file shows the ErreurChargement view instead of crashing or exiting silently.

Record what you actually observed for each bullet in your report — not just "it worked."

- [ ] **Step 4: Commit**

```bash
git add crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): wire worker, protocol, and views into a working desktop app"
```
