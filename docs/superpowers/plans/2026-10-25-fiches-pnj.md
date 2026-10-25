# Fiches PNJ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the game-master stay consistent with recurring NPCs it invents during free narration — a new domain type `FichePnj` (name, description, encounter history) persists on `EtatPartie`, created/updated by a dedicated LLM detection call after each non-combat turn's narration, and injected back into future narration prompts via deterministic name-matching against the player's own text.

**Architecture:** Two separate mechanisms, not one: deterministic injection (no LLM call — a known PNJ's name appearing in the player's free text pulls their name+description into that turn's narration prompt) runs *before* narration; LLM-backed detection/creation (one extra call, hors combat only) runs *after* narration, inspecting what was just narrated to create a new fiche or extend an existing one's history. This split exists specifically so a returning PNJ's fiche can inform the same turn's narration, which a purely post-hoc design could never do.

**Tech Stack:** Same Rust workspace (domain/application/infra/app), `llama-cpp-2` for the real LLM-backed call, `FauxNarrateur` as the test double.

**Spec:** `docs/superpowers/specs/2026-10-25-fiches-pnj-design.md`

## Global Constraints

- `FichePnj { nom: String, description: String, historique: Vec<String> }` (domain), serialized via `serde` like every other `EtatPartie` field.
- The encounter `historique` is **never** reinjected into any prompt — only `nom` + `description`. This is deliberate: it avoids reproducing the unbounded-token-growth bug that `resumes` had before it was fixed.
- Injection (into the narration prompt) is deterministic, case-insensitive substring matching of a known PNJ's `nom` against the player's free text — **no LLM call** for this side.
- Injection only applies when `contexte.action` is `ActionJoueur::ActionLibre(texte)` — the only variant carrying free text.
- Detection/creation (the new `Narrateur::detecter_pnj` call) runs **only when there is no active combat** — reuse the existing private `combat_termine(etat: &EtatPartie) -> bool` helper in `jouer_tour.rs`, never reimplement the same check.
- A failed or empty detection result never fails the turn and is never surfaced to the player — same silent philosophy as `condenser_resumes`.
- `detecter_pnj` uses unconstrained generation (no GBNF grammar, already disabled project-wide since `4d7f9e4`) and is never streamed to the player.
- The classification prompt (`construire_prompt_classification`) is **not** touched by this plan.

---

### Task 1: Domain — `FichePnj` and threading `pnj_connus` through `EtatPartie`

**Files:**
- Create: `crates/domain/src/fiche_pnj.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/src/etat_partie.rs`
- Modify: `crates/application/src/ports/sauvegarde.rs:40`
- Modify: `crates/application/src/usecases/partie.rs:34`
- Modify: `crates/application/src/usecases/jouer_tour.rs:277`
- Modify: `crates/infra/src/faux_narrateur.rs:80`
- Modify: `crates/infra/src/action_schema.rs:130`
- Modify: `crates/infra/src/prompts.rs:133`
- Modify: `crates/infra/src/llama_narrateur.rs:230`
- Modify: `crates/infra/src/sauvegarde_fichier.rs:41`
- Modify: `crates/app/src/main.rs:34`

**Interfaces:**
- Produces: `domain::FichePnj { nom: String, description: String, historique: Vec<String> }` (public, re-exported from `domain`), and a new field `EtatPartie.pnj_connus: Vec<FichePnj>`. Every later task in this plan constructs/reads this field.

Adding a field to `EtatPartie` breaks compilation of every existing `EtatPartie { ... }` struct literal in the workspace until each one adds the new field — there are 11 such literals (one production site in `app`, ten test helpers). This task's own test gate is `cargo test -p domain` (self-contained); the workspace as a whole does not compile again until Step 4 below updates every other literal.

- [ ] **Step 1: Write the failing test**

Open `crates/domain/src/etat_partie.rs`. Replace the test module's `etat_test` helper and its one test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::caracteristiques::Caracteristiques;

    fn etat_test() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques {
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
            resumes: vec!["Le voyage commence.".into()],
            pnj_connus: vec![FichePnj {
                nom: "Aldric".into(),
                description: "forgeron bourru et méfiant".into(),
                historique: vec!["Première rencontre à la forge.".into()],
            }],
        }
    }

    #[test]
    fn etat_partie_fait_un_aller_retour_json() {
        let etat = etat_test();
        let json = serde_json::to_string(&etat).expect("sérialisation échouée");
        let relu: EtatPartie = serde_json::from_str(&json).expect("désérialisation échouée");
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
        assert_eq!(relu.lieu_actuel, etat.lieu_actuel);
        assert_eq!(relu.resumes, etat.resumes);
        assert_eq!(relu.pnj_connus, etat.pnj_connus);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p domain`

Expected: compile error — `FichePnj` does not exist yet, and `EtatPartie` has no field `pnj_connus`.

- [ ] **Step 3: Create `FichePnj`, export it, and add the field to `EtatPartie`**

Create `crates/domain/src/fiche_pnj.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FichePnj {
    pub nom: String,
    pub description: String,
    pub historique: Vec<String>,
}
```

Open `crates/domain/src/lib.rs`. Add `mod fiche_pnj;` next to the other `mod` declarations (alphabetically, between `ennemi` and `etat_partie`) and `pub use fiche_pnj::FichePnj;` next to the other `pub use` lines (same position):

```rust
mod action_joueur;
mod caracteristiques;
mod combat;
mod de;
mod ennemi;
mod etat_partie;
mod fiche_pnj;
mod objet;
mod personnage;

pub use action_joueur::ActionJoueur;
pub use caracteristiques::{modificateur, Caracteristiques};
pub use combat::{determiner_ordre_initiative, resoudre_attaque, Combat, ResolutionAttaque, Vainqueur};
pub use de::{De, JetDe};
pub use ennemi::Ennemi;
pub use etat_partie::EtatPartie;
pub use fiche_pnj::FichePnj;
pub use objet::{EffetObjet, Objet};
pub use personnage::Personnage;
```

Open `crates/domain/src/etat_partie.rs`. Replace the struct definition:

```rust
use crate::combat::Combat;
use crate::fiche_pnj::FichePnj;
use crate::personnage::Personnage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
    pub pnj_connus: Vec<FichePnj>,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p domain`

Expected: PASS — `domain`'s own test suite is self-contained and now compiles and passes again. The rest of the workspace does not compile yet; that's expected and fixed in the next step.

- [ ] **Step 5: Update every other `EtatPartie` struct literal in the workspace**

Each of the following needs exactly one line added, `pnj_connus: vec![],` (or `Vec::new()` where the surrounding file already uses that style) immediately after the existing `resumes: ...,` line in that same literal.

`crates/application/src/ports/sauvegarde.rs:40` — in `fn etat_test()`, the literal currently ends:
```rust
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
        }
```
becomes:
```rust
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
        }
```

`crates/application/src/usecases/partie.rs:34` — same shape, same fix (`fn etat_test()`).

`crates/application/src/usecases/jouer_tour.rs:277` — in `fn etat_sans_combat()`, same shape, same fix. (`etat_en_combat()` calls `etat_sans_combat()` and only adds `combat_en_cours`, so it needs no separate edit.)

`crates/infra/src/faux_narrateur.rs:80` — inside `fn contexte(...)`, the literal is nested one level deeper (`ContexteTour { etat: EtatPartie { ... }, saisie_joueur: ... }`); same one-line fix inside the inner `EtatPartie { ... }`.

`crates/infra/src/action_schema.rs:130` — in `fn etat_sans_rien()`, same shape, same fix.

`crates/infra/src/prompts.rs:133` — in `fn etat_sans_combat()`, same shape, same fix. (`etat_en_combat()` there also calls `etat_sans_combat()`, no separate edit needed.)

`crates/infra/src/llama_narrateur.rs:230` — in `fn etat_sans_rien()`, same shape, same fix.

`crates/infra/src/sauvegarde_fichier.rs:41` — in `fn etat_test()`, same shape, same fix.

`crates/app/src/main.rs:34` — this is the one **production** (non-test) site, inside the `Err(ErreurSauvegarde::FichierAbsent)` branch of `main()`. Current code:
```rust
        Err(ErreurSauvegarde::FichierAbsent) => {
            let personnage = creation::creer_personnage_interactif();
            let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
            let combat = commencer_combat(&personnage, vec![gobelin], &mut de);
            EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: Some(combat),
                resumes: Vec::new(),
            }
        }
```
becomes:
```rust
        Err(ErreurSauvegarde::FichierAbsent) => {
            let personnage = creation::creer_personnage_interactif();
            let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
            let combat = commencer_combat(&personnage, vec![gobelin], &mut de);
            EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: Some(combat),
                resumes: Vec::new(),
                pnj_connus: Vec::new(),
            }
        }
```

- [ ] **Step 6: Run the full workspace test suite to verify it passes**

Run: `cargo test --workspace`

Expected: PASS — every crate compiles and every existing test still passes (no new behavior yet, just the new field threaded through).

- [ ] **Step 7: Commit**

```bash
git add crates/domain/src/fiche_pnj.rs crates/domain/src/lib.rs crates/domain/src/etat_partie.rs crates/application/src/ports/sauvegarde.rs crates/application/src/usecases/partie.rs crates/application/src/usecases/jouer_tour.rs crates/infra/src/faux_narrateur.rs crates/infra/src/action_schema.rs crates/infra/src/prompts.rs crates/infra/src/llama_narrateur.rs crates/infra/src/sauvegarde_fichier.rs crates/app/src/main.rs
git commit -m "feat(domain): add FichePnj and thread pnj_connus through EtatPartie"
```

---

### Task 2: Application — `detecter_pnj` port method and `JouerTour` wiring

**Files:**
- Modify: `crates/application/src/ports/narrateur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`

**Interfaces:**
- Consumes: `domain::FichePnj` (Task 1), the existing `Narrateur` trait (`proposer_action`, `raconter_resultat`, `condenser_resumes`), the existing private `combat_termine(etat: &EtatPartie) -> bool` helper already in `jouer_tour.rs`.
- Produces: `pub struct PnjDetecte { pub nom: String, pub nouvelle_description: Option<String> }` and `Narrateur::detecter_pnj(&mut self, narration: &str, noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur>` — `infra`'s implementors are updated in Task 3.

This task touches only `application`, which does not depend on `infra` — `cargo test -p application` is a valid, fully isolated gate even though `infra`'s two `Narrateur` implementors won't compile again until Task 3, mirroring the résumés plan's Task 1/Task 2 split.

- [ ] **Step 1: Write the failing tests**

Open `crates/application/src/usecases/jouer_tour.rs`. First, add `FichePnj` to the top-level `domain` import:

```rust
use domain::{resoudre_attaque, ActionJoueur, De, EffetObjet, EtatPartie, FichePnj, Vainqueur};
```

In the `#[cfg(test)] mod tests` block, add `detecter_pnj` to the three existing test doubles (`NarrateurDeTest`, `NarrateurEspion`, `NarrateurCondensateur`) so the file keeps compiling once the trait gains the new method — each returns `Ok(None)` (no PNJ detected), which is what every existing test already implicitly expects:

Add to `impl Narrateur for NarrateurDeTest`:
```rust
        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }
```

Add to `impl Narrateur for NarrateurEspion`:
```rust
        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }
```

Add to `impl Narrateur for NarrateurCondensateur`:
```rust
        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }
```

Replace the existing port import line (`use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};`) with the same line plus `PnjDetecte`:
```rust
    use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};
```

Then add a new test double, right after `NarrateurCondensateur`'s `impl` block:

```rust
    struct NarrateurDetectionPnj {
        resultat: Result<Option<PnjDetecte>, ErreurNarrateur>,
    }

    impl Narrateur for NarrateurDetectionPnj {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok("Aldric le forgeron te salue.".to_string())
        }

        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            match &self.resultat {
                Ok(valeur) => Ok(valeur.clone()),
                Err(ErreurNarrateur::EchecGeneration(message)) => {
                    Err(ErreurNarrateur::EchecGeneration(message.clone()))
                }
            }
        }
    }

    #[test]
    fn jouer_tour_cree_une_fiche_pnj_pour_un_nouveau_pnj_detecte() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru et méfiant".to_string()),
            })),
        };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je parle au forgeron", &mut narrateur, &mut de, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus.len(), 1);
        assert_eq!(etat.pnj_connus[0].nom, "Aldric");
        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
        assert_eq!(etat.pnj_connus[0].historique, vec!["Aldric le forgeron te salue.".to_string()]);
    }

    #[test]
    fn jouer_tour_ajoute_a_l_historique_d_une_fiche_pnj_existante() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru et méfiant".to_string(),
            historique: vec!["Première rencontre à la forge.".to_string()],
        });
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })),
        };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je retourne voir Aldric", &mut narrateur, &mut de, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus.len(), 1);
        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
        assert_eq!(
            etat.pnj_connus[0].historique,
            vec!["Première rencontre à la forge.".to_string(), "Aldric le forgeron te salue.".to_string()]
        );
    }

    #[test]
    fn jouer_tour_ne_detecte_pas_de_pnj_pendant_un_combat_actif() {
        let mut etat = etat_en_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru".to_string()),
            })),
        };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3
        // le gobelin (7 pv) survit, il agit ensuite : d20=1 + bonus_attaque 3 = 4 < défense 12 (manqué)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3, 1, 1]);
        JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de, &mut |_| {}).unwrap();

        assert!(etat.pnj_connus.is_empty());
    }

    #[test]
    fn jouer_tour_laisse_pnj_connus_inchange_si_la_detection_echoue() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Err(ErreurNarrateur::EchecGeneration("échec simulé".into())),
        };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {});

        assert!(resultat.is_ok());
        assert!(etat.pnj_connus.is_empty());
    }
```

Here `narrateur` in the combat test is `NarrateurDetectionPnj` acting as the full `Narrateur` for that call (it also supplies `proposer_action`/`raconter_resultat`, which this test doesn't care about beyond letting the turn complete). Its `detecter_pnj` would return a PNJ if called, so the only thing that can prevent `etat.pnj_connus` from gaining an entry is the combat-active skip in `enregistrer_pnj_si_pertinent` itself — a true test of that guard, not a trivial pass from some other double's default `Ok(None)`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p application jouer_tour`

Expected: compile error — `detecter_pnj`/`PnjDetecte` don't exist on the `Narrateur` port yet.

- [ ] **Step 3: Add `PnjDetecte` and `detecter_pnj` to the `Narrateur` port**

Open `crates/application/src/ports/narrateur.rs`. Add the new struct, right after `ContexteResultat`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PnjDetecte {
    pub nom: String,
    pub nouvelle_description: Option<String>,
}
```

Add a 5th numbered rule to the trait's doc comment, right after rule 4:

```rust
/// 5. `detecter_pnj` must behave as a pure detection/extraction step over
///    the narration text it's given: it never inspects or mutates
///    `EtatPartie` directly, and the decision of whether a PNJ is already
///    known (by matching `noms_connus`) versus new is `application`'s to
///    act on, not the implementation's to enforce.
```

Add the method to the trait itself, after `condenser_resumes`:

```rust
    /// Detects whether a PNJ (new or already known) appeared in the
    /// narration text just produced. `noms_connus` lets the model match a
    /// returning PNJ instead of inventing a duplicate. Never shown to the
    /// player, no streaming.
    fn detecter_pnj(
        &mut self,
        narration: &str,
        noms_connus: &[String],
    ) -> Result<Option<PnjDetecte>, ErreurNarrateur>;
```

- [ ] **Step 4: Wire detection/creation into `JouerTour::executer`**

Open `crates/application/src/usecases/jouer_tour.rs`. Add a private helper function right after `condenser_si_necessaire`:

```rust
fn enregistrer_pnj_si_pertinent(etat: &mut EtatPartie, narration: &str, narrateur: &mut dyn Narrateur) {
    if !combat_termine(etat) {
        return;
    }

    let noms_connus: Vec<String> = etat.pnj_connus.iter().map(|p| p.nom.clone()).collect();
    if let Ok(Some(detecte)) = narrateur.detecter_pnj(narration, &noms_connus) {
        let fiche_existante = etat
            .pnj_connus
            .iter_mut()
            .find(|p| p.nom.eq_ignore_ascii_case(&detecte.nom));
        if let Some(fiche) = fiche_existante {
            fiche.historique.push(narration.to_string());
        } else {
            etat.pnj_connus.push(FichePnj {
                nom: detecte.nom,
                description: detecte.nouvelle_description.unwrap_or_default(),
                historique: vec![narration.to_string()],
            });
        }
    }
}
```

In `JouerTour::executer`, right after the existing `condenser_si_necessaire(etat, narrateur);` line, add:

```rust
        enregistrer_pnj_si_pertinent(etat, &narration, narrateur);
```

So the end of `executer` reads:

```rust
        etat.resumes.push(narration.clone());
        condenser_si_necessaire(etat, narrateur);
        enregistrer_pnj_si_pertinent(etat, &narration, narrateur);

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            combat_termine: etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(false),
        })
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p application`

Expected: PASS — all existing tests still pass, plus the four new tests from Step 1 (three creation/update/failure tests, plus the combat-skip test using the simplified version).

- [ ] **Step 6: Commit**

```bash
git add crates/application/src/ports/narrateur.rs crates/application/src/usecases/jouer_tour.rs
git commit -m "feat(application): add detecter_pnj port method and wire fiche creation/update into JouerTour"
```

---

### Task 3: Infra — detection/injection prompts, parsing, and both `Narrateur` implementors

**Files:**
- Modify: `crates/infra/src/prompts.rs`
- Create: `crates/infra/src/detection_pnj.rs`
- Modify: `crates/infra/src/lib.rs`
- Modify: `crates/infra/src/faux_narrateur.rs`
- Modify: `crates/infra/src/llama_narrateur.rs`

**Interfaces:**
- Consumes: `Narrateur::detecter_pnj` and `PnjDetecte` (Task 2), `domain::FichePnj` (Task 1), `LlamaNarrateur::generer` (existing private method, unchanged signature).
- Produces: `pub fn construire_prompt_detection_pnj(narration: &str, noms_connus: &[String]) -> String` and `pub fn interpreter_json_detection_pnj(json_brut: &str) -> Result<Option<PnjDetecte>, ErreurInterpretationPnj>`, both consumed by `LlamaNarrateur::detecter_pnj` in this same task.

All three `infra` files (`prompts.rs`, `faux_narrateur.rs`, `llama_narrateur.rs`) plus the new `detection_pnj.rs` are compilation-coupled within the `infra` crate exactly like the résumés plan's Task 2 — `cargo test -p infra` only becomes valid again once every `impl Narrateur` block in the crate has `detecter_pnj`. Do all of this task's changes before running any test.

- [ ] **Step 1: Write the failing tests for the detection/injection prompts**

Open `crates/infra/src/prompts.rs`. First, add `ActionJoueur` and `FichePnj` to the top-level `domain` import:

```rust
use domain::{ActionJoueur, EffetObjet, EtatPartie, FichePnj};
```

In the `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn le_prompt_de_narration_contient_la_fiche_d_un_pnj_mentionne_par_le_joueur() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru et méfiant".to_string(),
            historique: vec![],
        });
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre("Je parle à Aldric".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Aldric"));
        assert!(prompt.contains("forgeron bourru et méfiant"));
    }

    #[test]
    fn le_prompt_de_narration_ne_mentionne_pas_un_pnj_non_pertinent() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru".to_string(),
            historique: vec![],
        });
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre("Je regarde le ciel".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.contains("Aldric"));
    }

    #[test]
    fn le_prompt_de_detection_pnj_contient_la_narration_et_les_noms_connus() {
        let prompt = construire_prompt_detection_pnj("Aldric te salue.", &["Aldric".to_string()]);
        assert!(prompt.contains("Aldric te salue."));
        assert!(prompt.contains("Aldric"));
    }

    #[test]
    fn le_prompt_de_detection_pnj_gere_une_liste_de_noms_connus_vide() {
        let prompt = construire_prompt_detection_pnj("Rien ne se passe.", &[]);
        assert!(prompt.contains("aucun"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p infra prompts`

Expected: compile error — `construire_prompt_detection_pnj` doesn't exist yet.

- [ ] **Step 3: Implement `description_pnj_pertinents`, `construire_prompt_detection_pnj`, and wire the injection into `construire_prompt_narration`**

Still in `crates/infra/src/prompts.rs`, add this new public function, right after `construire_prompt_condensation`:

```rust
pub fn construire_prompt_detection_pnj(narration: &str, noms_connus: &[String]) -> String {
    format!(
        "Voici ce qui vient d'être raconté : {}\n\
         PNJ déjà connus : {}\n\
         Un personnage non-joueur important est-il apparu ou a-t-il été mentionné dans ce texte ? \
         Réponds uniquement par un objet JSON, sans aucun autre texte : \
         {{\"pnj\": null}} si aucun, {{\"pnj\": \"Nom\", \"nouveau\": false}} si c'est un PNJ déjà \
         connu, ou {{\"pnj\": \"Nom\", \"nouveau\": true, \"description\": \"...\"}} si c'est un \
         nouveau PNJ (description en quelques mots : personnalité, rôle).",
        narration,
        if noms_connus.is_empty() { "aucun".to_string() } else { noms_connus.join(", ") },
    )
}
```

Add this private helper, right after `description_combat`:

```rust
fn description_pnj_pertinents(etat: &EtatPartie, action: &ActionJoueur) -> String {
    let ActionJoueur::ActionLibre(texte) = action else {
        return String::new();
    };
    let texte_minuscule = texte.to_lowercase();
    let pertinents: Vec<&FichePnj> = etat
        .pnj_connus
        .iter()
        .filter(|p| texte_minuscule.contains(&p.nom.to_lowercase()))
        .collect();
    if pertinents.is_empty() {
        String::new()
    } else {
        let descriptions: Vec<String> = pertinents
            .iter()
            .map(|p| format!("{} : {}", p.nom, p.description))
            .collect();
        format!("PNJ pertinents déjà rencontrés : {}\n", descriptions.join(" ; "))
    }
}
```

Finally, modify `construire_prompt_narration` to include it. Replace:

```rust
pub fn construire_prompt_narration(contexte: &ContexteResultat) -> String {
    let resultat = if contexte.resume_mecanique.is_empty() {
        "Rien de mécanique ne s'est produit.".to_string()
    } else {
        contexte.resume_mecanique.join(" ")
    };
    format!(
        "{}\
         Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        description_memoire(&contexte.etat), contexte.action, resultat, description_etat_apres_resolution(&contexte.etat),
    )
}
```

with:

```rust
pub fn construire_prompt_narration(contexte: &ContexteResultat) -> String {
    let resultat = if contexte.resume_mecanique.is_empty() {
        "Rien de mécanique ne s'est produit.".to_string()
    } else {
        contexte.resume_mecanique.join(" ")
    };
    format!(
        "{}\
         {}\
         Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        description_memoire(&contexte.etat),
        description_pnj_pertinents(&contexte.etat, &contexte.action),
        contexte.action,
        resultat,
        description_etat_apres_resolution(&contexte.etat),
    )
}
```

- [ ] **Step 4: Run the prompt tests — expect a compile failure until the next steps are done**

Run: `cargo test -p infra prompts`

Expected: still fails to compile, because `faux_narrateur.rs` and `llama_narrateur.rs` in the same crate don't implement `detecter_pnj` yet. This is expected — continue to Steps 5-7 before re-running.

- [ ] **Step 5: Create `detection_pnj.rs` with the JSON parser**

Create `crates/infra/src/detection_pnj.rs`:

```rust
use application::ports::narrateur::PnjDetecte;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct ErreurInterpretationPnj(pub String);

pub fn interpreter_json_detection_pnj(json_brut: &str) -> Result<Option<PnjDetecte>, ErreurInterpretationPnj> {
    let valeur: Value = serde_json::from_str(json_brut)
        .map_err(|e| ErreurInterpretationPnj(format!("JSON invalide : {e}")))?;

    let pnj = valeur
        .get("pnj")
        .ok_or_else(|| ErreurInterpretationPnj("champ \"pnj\" manquant".to_string()))?;

    if pnj.is_null() {
        return Ok(None);
    }

    let nom = pnj
        .as_str()
        .ok_or_else(|| ErreurInterpretationPnj("champ \"pnj\" doit être une chaîne ou null".to_string()))?
        .to_string();

    let nouveau = valeur.get("nouveau").and_then(Value::as_bool).unwrap_or(false);

    let nouvelle_description = if nouveau {
        Some(
            valeur
                .get("description")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ErreurInterpretationPnj("champ \"description\" manquant pour un nouveau PNJ".to_string())
                })?
                .to_string(),
        )
    } else {
        None
    };

    Ok(Some(PnjDetecte { nom, nouvelle_description }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprete_l_absence_de_pnj() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": null}"#).unwrap();
        assert_eq!(resultat, None);
    }

    #[test]
    fn interprete_un_pnj_deja_connu() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": false}"#).unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })
        );
    }

    #[test]
    fn interprete_un_nouveau_pnj_avec_sa_description() {
        let resultat =
            interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": true, "description": "forgeron bourru"}"#)
                .unwrap();
        assert_eq!(
            resultat,
            Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru".to_string())
            })
        );
    }

    #[test]
    fn un_nouveau_pnj_sans_description_echoue() {
        let resultat = interpreter_json_detection_pnj(r#"{"pnj": "Aldric", "nouveau": true}"#);
        assert!(resultat.is_err());
    }

    #[test]
    fn un_json_invalide_echoue() {
        let resultat = interpreter_json_detection_pnj("ceci n'est pas du json");
        assert!(resultat.is_err());
    }

    #[test]
    fn un_champ_pnj_manquant_echoue() {
        let resultat = interpreter_json_detection_pnj(r#"{"autre_chose": true}"#);
        assert!(resultat.is_err());
    }
}
```

Open `crates/infra/src/lib.rs` and add `mod detection_pnj;` next to the other `mod` declarations (alphabetically, between `action_schema` and `faux_narrateur`):

```rust
mod action_schema;
mod detection_pnj;
mod faux_narrateur;
mod generateur_de_os;
mod llama_narrateur;
mod prompts;
mod sauvegarde_fichier;

pub use faux_narrateur::FauxNarrateur;
pub use generateur_de_os::GenerateurDeOs;
pub use llama_narrateur::LlamaNarrateur;
pub use sauvegarde_fichier::SauvegardeFichier;
```

(No `pub use` needed for `detection_pnj` — nothing outside `infra` needs it by name, same as `action_schema`.)

- [ ] **Step 6: Implement `detecter_pnj` on `FauxNarrateur`**

Open `crates/infra/src/faux_narrateur.rs`. Add this method to `impl Narrateur for FauxNarrateur`, right after `condenser_resumes`:

```rust
    fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
        Ok(None)
    }
```

Update the top-of-file import to bring in `PnjDetecte`:

```rust
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};
```

Add this test to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn la_detection_de_pnj_ne_renvoie_rien_par_defaut() {
        let mut narrateur = FauxNarrateur;
        let resultat = narrateur.detecter_pnj("Peu importe.", &[]).unwrap();
        assert_eq!(resultat, None);
    }
```

- [ ] **Step 7: Implement `detecter_pnj` on `LlamaNarrateur`**

Open `crates/infra/src/llama_narrateur.rs`. Update the imports at the top of the file:

```rust
use crate::action_schema::interpreter_json_action;
use crate::detection_pnj::interpreter_json_detection_pnj;
use crate::prompts::{
    construire_prompt_classification, construire_prompt_condensation, construire_prompt_detection_pnj,
    construire_prompt_narration,
};
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};
use domain::ActionJoueur;
```

Add a new constant next to the other token-budget constants:

```rust
const TOKENS_MAX_DETECTION_PNJ: i32 = 100;
```

Add the method to `impl Narrateur for LlamaNarrateur`, right after `condenser_resumes`:

```rust
    fn detecter_pnj(&mut self, narration: &str, noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
        let prompt = construire_prompt_detection_pnj(narration, noms_connus);
        let json_brut = self
            .generer(&prompt, TOKENS_MAX_DETECTION_PNJ, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)?;
        interpreter_json_detection_pnj(&json_brut).map_err(|e| ErreurNarrateur::EchecGeneration(e.0))
    }
```

No automated test for this method — same reasoning as `proposer_action`/`raconter_resultat`/`condenser_resumes` (real model, non-deterministic): verified manually in Step 9.

- [ ] **Step 8: Run the full infra test suite and the full workspace test suite**

Run: `cargo test -p infra`

Expected: PASS — all `prompts.rs` tests (including the 4 new ones from Step 1), all `detection_pnj.rs` tests (6 new ones from Step 5), and the new `faux_narrateur.rs` test pass.

Run: `cargo test --workspace`

Expected: PASS — the whole workspace compiles and all tests pass.

- [ ] **Step 9: Manual end-to-end verification with the real model**

Run a game session against a real model to confirm a PNJ fiche actually gets created and correctly reused:

```bash
NARRATEUR_MODELE=/home/alexandre/dev/narrator/modeles/Qwen2.5-3B-Instruct-Q4_K_M.gguf cargo run --release -p app
```

(Using the hand-crafted no-active-combat save technique established during the résumés plan's manual verification is fine here too, if the forced starting combat gets in the way — write a `partie.json` with `combat_en_cours: null` and `pnj_connus: []` before launching, matching the existing `EtatPartie` JSON shape.)

Play several non-combat, free-text turns designed to introduce a named NPC (e.g. "Je m'approche de la forge et je parle au forgeron"). After a turn where the narration introduces someone by name, inspect `partie.json` and confirm `pnj_connus` gained one entry with a name and description. Then, in a later turn, explicitly mention that name again (e.g. "Je retourne voir Aldric") and confirm: (a) the narration prompt now includes that PNJ's description (not directly observable from the CLI, but you can confirm indirectly — the narration should stay consistent with the established personality rather than contradicting it), and (b) `pnj_connus` still has exactly one entry for that name (not duplicated), with `historique` now containing two entries.

- [ ] **Step 10: Commit**

```bash
git add crates/infra/src/prompts.rs crates/infra/src/detection_pnj.rs crates/infra/src/lib.rs crates/infra/src/faux_narrateur.rs crates/infra/src/llama_narrateur.rs
git commit -m "feat(infra): implement PNJ detection/injection prompts, parsing, and both Narrateur implementors"
```
