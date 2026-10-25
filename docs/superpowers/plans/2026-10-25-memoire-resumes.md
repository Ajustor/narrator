# Résumés automatiques de mémoire Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `EtatPartie.resumes` (already in the domain, currently always empty) real behavior: every turn's narration is appended to it, and once the accumulated text crosses a word threshold, older entries are condensed by the LLM into one shorter entry — so the narrator keeps long-term continuity without the prompt growing without bound.

**Architecture:** The `Narrateur` port gains a third method, `condenser_resumes`, that compresses a slice of résumé strings into one. `application` decides the cut point between "recent, kept verbatim" and "older, sent for condensation" — the LLM only compresses text it's handed, same separation of concerns as the existing `proposer_action`/`raconter_resultat` split (dice/state decisions stay in code, the LLM only narrates/compresses text).

**Tech Stack:** Rust workspace (`domain`/`application`/`infra`/`app`), `llama-cpp-2` for the real LLM-backed implementation (`LlamaNarrateur`), `FauxNarrateur` as the test double.

**Spec:** `docs/superpowers/specs/2026-10-25-memoire-resumes-design.md`

## Global Constraints

- `SEUIL_MOTS_RESUMES: usize = 800` — total word count across `etat.resumes` above which condensation triggers.
- `NB_RESUMES_RECENTS_CONSERVES: usize = 5` — number of most-recent résumés kept verbatim (never condensed); only entries older than these are sent to `condenser_resumes`.
- Word count, not token count — `application` has no access to any tokenizer (that lives in `infra`).
- Condensation is invisible to the player in both outcomes (success or failure) — no message, no log line in `app`.
- A failed condensation must never fail the turn — the turn's narration is still returned normally; `etat.resumes` is simply left over-threshold, to be retried next turn.
- The classification prompt (`construire_prompt_classification`) is **not** touched by this plan — résumés are injected only into the narration prompt.
- `condenser_resumes` uses unconstrained generation (no GBNF grammar — already disabled project-wide since `4d7f9e4`) and is never streamed to the player.
- Fiches PNJ are out of scope for this plan (separate future sub-project).

---

### Task 1: Extend the `Narrateur` port and wire résumé accumulation/condensation into `JouerTour`

**Files:**
- Modify: `crates/application/src/ports/narrateur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`

**Interfaces:**
- Consumes: existing `Narrateur` trait (`proposer_action`, `raconter_resultat`), existing `ContexteResultat`/`ContexteTour`, existing `EtatPartie.resumes: Vec<String>` (domain, already present).
- Produces: `Narrateur::condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur>` (new trait method — `infra`'s implementors are updated in Task 2); `pub const SEUIL_MOTS_RESUMES: usize` and `pub const NB_RESUMES_RECENTS_CONSERVES: usize` in `jouer_tour.rs`, consumed nowhere outside this task but part of the public contract future tasks/tests may reference.

This task only touches `application`, which does not depend on `infra` — `cargo test -p application` is a valid, fully isolated gate for this task even though `infra`'s two `Narrateur` implementors (`FauxNarrateur`, `LlamaNarrateur`) won't compile again until Task 2 adds the new method to them. This mirrors how the port was split in the previous LLM-narrateur plan (the port changed before every implementor caught up).

- [ ] **Step 1: Write the failing tests for résumé accumulation and condensation**

Open `crates/application/src/usecases/jouer_tour.rs`. In the `#[cfg(test)] mod tests` block, find the existing `NarrateurDeTest` and `NarrateurEspion` test doubles (they currently implement only `proposer_action`/`raconter_resultat`). Add a third test double, `NarrateurCondensateur`, right after `NarrateurEspion`'s `impl Narrateur for NarrateurEspion` block:

```rust
    struct NarrateurCondensateur {
        resultat: Result<String, ErreurNarrateur>,
        anciens_recus: RefCell<Option<Vec<String>>>,
    }

    impl Narrateur for NarrateurCondensateur {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok("Il ne se passe rien de notable.".to_string())
        }

        fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
            *self.anciens_recus.borrow_mut() = Some(anciens.to_vec());
            match &self.resultat {
                Ok(texte) => Ok(texte.clone()),
                Err(ErreurNarrateur::EchecGeneration(message)) => {
                    Err(ErreurNarrateur::EchecGeneration(message.clone()))
                }
            }
        }
    }

    fn resume_de_n_mots(n: usize) -> String {
        "mot ".repeat(n).trim_end().to_string()
    }

    #[test]
    fn jouer_tour_ajoute_la_narration_aux_resumes() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert_eq!(etat.resumes, vec!["narration de test".to_string()]);
    }

    #[test]
    fn jouer_tour_condense_les_anciens_resumes_une_fois_le_seuil_depasse() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..9).map(|_| resume_de_n_mots(90)).collect(); // 9 * 90 = 810 mots, déjà au-dessus du seuil
        let mut narrateur = NarrateurCondensateur {
            resultat: Ok("résumé condensé".to_string()),
            anciens_recus: RefCell::new(None),
        };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {}).unwrap();

        // 9 anciens + 1 nouvelle narration = 10 résumés avant condensation ;
        // les NB_RESUMES_RECENTS_CONSERVES=5 derniers sont gardés, les 5 plus anciens condensés.
        assert_eq!(etat.resumes.len(), 1 + NB_RESUMES_RECENTS_CONSERVES);
        assert_eq!(etat.resumes[0], "résumé condensé");

        let anciens_recus = narrateur.anciens_recus.borrow();
        assert_eq!(anciens_recus.as_ref().unwrap().len(), 5);
    }

    #[test]
    fn jouer_tour_laisse_les_resumes_inchanges_si_la_condensation_echoue() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..9).map(|_| resume_de_n_mots(90)).collect();
        let resumes_avant = etat.resumes.clone();
        let mut narrateur = NarrateurCondensateur {
            resultat: Err(ErreurNarrateur::EchecGeneration("échec simulé".into())),
            anciens_recus: RefCell::new(None),
        };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {});

        assert!(resultat.is_ok());
        let mut resumes_attendus = resumes_avant;
        resumes_attendus.push("Il ne se passe rien de notable.".to_string());
        assert_eq!(etat.resumes, resumes_attendus);
    }
```

Also update the two existing test doubles to implement the new trait method (needed simply for the file to compile once Step 3 adds `condenser_resumes` to the trait) — add this method to `impl Narrateur for NarrateurDeTest`:

```rust
        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok("résumé condensé de test".to_string())
        }
```

and this one to `impl Narrateur for NarrateurEspion`:

```rust
        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p application jouer_tour`

Expected: compile error — `condenser_resumes` does not exist on the `Narrateur` trait yet (`NarrateurCondensateur`'s `impl Narrateur` references a method the trait doesn't declare), and the new assertions in the three new tests reference behavior (`etat.resumes` accumulation/condensation) that doesn't exist yet.

- [ ] **Step 3: Add `condenser_resumes` to the `Narrateur` port**

Open `crates/application/src/ports/narrateur.rs`. Add a 4th numbered rule to the trait's doc comment, right after rule 3 (before `pub trait Narrateur`):

```rust
/// 4. `condenser_resumes` must behave as a pure text-compression step: it
///    never inspects or mutates `EtatPartie`, and the decision of which
///    résumés are "old enough" to send to it is entirely `application`'s
///    responsibility, not the implementation's.
```

Then add the method to the trait itself, after `raconter_resultat`:

```rust
    /// Compresses older turn-résumés (oldest to newest) into a single
    /// shorter string, replacing them. Never shown to the player, no
    /// streaming — same as `proposer_action`.
    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur>;
```

- [ ] **Step 4: Add the constants and the condensation logic to `JouerTour::executer`**

Open `crates/application/src/usecases/jouer_tour.rs`. Add the two constants near the top of the file, right after the existing `use` statements:

```rust
pub const SEUIL_MOTS_RESUMES: usize = 800;
pub const NB_RESUMES_RECENTS_CONSERVES: usize = 5;
```

Add a private helper function right before `pub struct JouerTour;`:

```rust
fn condenser_si_necessaire(etat: &mut EtatPartie, narrateur: &mut dyn Narrateur) {
    let total_mots: usize = etat.resumes.iter().map(|r| r.split_whitespace().count()).sum();
    if total_mots <= SEUIL_MOTS_RESUMES || etat.resumes.len() <= NB_RESUMES_RECENTS_CONSERVES {
        return;
    }

    let point_de_coupe = etat.resumes.len() - NB_RESUMES_RECENTS_CONSERVES;
    let anciens = &etat.resumes[..point_de_coupe];
    if let Ok(condense) = narrateur.condenser_resumes(anciens) {
        let mut nouveaux = vec![condense];
        nouveaux.extend_from_slice(&etat.resumes[point_de_coupe..]);
        etat.resumes = nouveaux;
    }
}
```

Finally, in `JouerTour::executer`, right after the line that assigns `narration` (the call to `narrateur.raconter_resultat(...)`) and before the final `Ok(ResultatTour { ... })`, add:

```rust
        etat.resumes.push(narration.clone());
        condenser_si_necessaire(etat, narrateur);
```

So the end of `executer` reads:

```rust
        let narration = narrateur
            .raconter_resultat(&contexte_resultat, sur_fragment)
            .map_err(message_echec_narrateur)?;

        etat.resumes.push(narration.clone());
        condenser_si_necessaire(etat, narrateur);

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            combat_termine: etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(false),
        })
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p application`

Expected: PASS — all existing tests in `application` still pass (unaffected), plus the three new tests from Step 1.

- [ ] **Step 6: Commit**

```bash
git add crates/application/src/ports/narrateur.rs crates/application/src/usecases/jouer_tour.rs
git commit -m "feat(application): accumulate and condense turn résumés in JouerTour"
```

---

### Task 2: Condensation prompt + both `Narrateur` implementors

**Files:**
- Modify: `crates/infra/src/prompts.rs`
- Modify: `crates/infra/src/faux_narrateur.rs`
- Modify: `crates/infra/src/llama_narrateur.rs`

**Interfaces:**
- Consumes: `Narrateur::condenser_resumes` (Task 1), `EtatPartie.resumes: Vec<String>` (domain), `LlamaNarrateur::generer` (existing private method, unchanged signature: `fn generer(&self, prompt: &str, max_tokens: i32, grammaire: Option<&str>, sur_fragment: &mut dyn FnMut(&str)) -> Result<String, String>`).
- Produces: `pub fn construire_prompt_condensation(anciens: &[String]) -> String` in `prompts.rs`, consumed by `LlamaNarrateur::condenser_resumes` in this same task.

These three files all live in the `infra` crate, and the crate does not compile as a whole until every `impl Narrateur` block in it has all three trait methods — so this task is one unit: splitting it across sub-tasks would leave `infra` (and anything depending on it) uncompilable in between. `cargo test -p infra` and `cargo test --workspace` are both valid gates only at the end of this task.

- [ ] **Step 1: Write the failing tests for the condensation prompt and the memory section of the narration prompt**

Open `crates/infra/src/prompts.rs`. In the `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn le_prompt_de_condensation_contient_les_resumes_fournis() {
        let anciens = vec!["Elowen quitte le village.".to_string(), "Elowen combat un gobelin.".to_string()];
        let prompt = construire_prompt_condensation(&anciens);
        assert!(prompt.contains("Elowen quitte le village."));
        assert!(prompt.contains("Elowen combat un gobelin."));
    }

    #[test]
    fn le_prompt_de_condensation_demande_un_resultat_court_et_en_francais() {
        let prompt = construire_prompt_condensation(&["Un résumé.".to_string()]);
        assert!(prompt.contains("en français"));
        assert!(prompt.contains("court"));
    }

    #[test]
    fn le_prompt_de_narration_contient_les_resumes_accumules() {
        let mut etat = etat_sans_combat();
        etat.resumes = vec!["Elowen a quitté le village.".to_string()];
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Elowen a quitté le village."));
    }

    #[test]
    fn le_prompt_de_narration_ne_mentionne_pas_de_memoire_si_resumes_est_vide() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.contains("Ce qui s'est passé jusqu'ici"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p infra prompts`

Expected: compile error — `construire_prompt_condensation` doesn't exist yet. (Once it's added, the two narration-prompt tests would still fail on assertion, since `etat.resumes` isn't in the prompt yet — but you won't see that far until the missing function is added; fix both in Step 3.)

- [ ] **Step 3: Implement `construire_prompt_condensation` and add the memory section to `construire_prompt_narration`**

Still in `crates/infra/src/prompts.rs`, add this new public function, right after `construire_prompt_narration`:

```rust
pub fn construire_prompt_condensation(anciens: &[String]) -> String {
    format!(
        "Voici plusieurs résumés successifs d'une histoire de jeu de rôle en français :\n\
         {}\n\
         Condense-les en un seul paragraphe court (3-4 phrases maximum) en français, \
         qui garde les faits importants (lieux visités, événements marquants, objets \
         obtenus, ennemis vaincus) sans les détails mineurs. Réponds uniquement par ce \
         paragraphe, sans aucun autre texte.",
        anciens.join("\n"),
    )
}
```

Then add this private helper, right after `description_combat`:

```rust
fn description_memoire(etat: &EtatPartie) -> String {
    if etat.resumes.is_empty() {
        String::new()
    } else {
        format!("Ce qui s'est passé jusqu'ici : {}\n", etat.resumes.join(" "))
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
        "Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        contexte.action, resultat, description_etat_apres_resolution(&contexte.etat),
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
         Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        description_memoire(&contexte.etat), contexte.action, resultat, description_etat_apres_resolution(&contexte.etat),
    )
}
```

- [ ] **Step 4: Run the prompt tests to verify they pass**

Run: `cargo test -p infra prompts`

Expected: still fails to compile at this point, because `faux_narrateur.rs` and `llama_narrateur.rs` in the same crate don't implement `condenser_resumes` yet. This is expected — continue to Step 5 before re-running.

- [ ] **Step 5: Implement `condenser_resumes` on `FauxNarrateur`**

Open `crates/infra/src/faux_narrateur.rs`. Add this method to `impl Narrateur for FauxNarrateur`, right after `raconter_resultat`:

```rust
    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
        Ok(format!("[résumé condensé de {} tours]", anciens.len()))
    }
```

Add this test to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn la_condensation_resume_le_nombre_de_tours_fournis() {
        let mut narrateur = FauxNarrateur;
        let condense = narrateur
            .condenser_resumes(&["Tour 1.".to_string(), "Tour 2.".to_string()])
            .unwrap();
        assert!(condense.contains('2'));
    }
```

- [ ] **Step 6: Implement `condenser_resumes` on `LlamaNarrateur`**

Open `crates/infra/src/llama_narrateur.rs`. Update the import at the top of the file from:

```rust
use crate::prompts::{construire_prompt_classification, construire_prompt_narration};
```

to:

```rust
use crate::prompts::{construire_prompt_classification, construire_prompt_condensation, construire_prompt_narration};
```

Add a new constant next to the two existing token-budget constants:

```rust
const TOKENS_MAX_CONDENSATION: i32 = 150;
```

Add the method to `impl Narrateur for LlamaNarrateur`, right after `raconter_resultat`:

```rust
    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_condensation(anciens);
        self.generer(&prompt, TOKENS_MAX_CONDENSATION, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)
    }
```

No automated test for this method — same reasoning as `proposer_action`/`raconter_resultat` (real model, non-deterministic): verified manually in Step 8.

- [ ] **Step 7: Run the full infra test suite and the full workspace test suite**

Run: `cargo test -p infra`

Expected: PASS — all `prompts.rs` tests (including the 4 new ones from Step 1) and the new `faux_narrateur.rs` test pass.

Run: `cargo test --workspace`

Expected: PASS — the whole workspace compiles and all tests pass (this is the point where `app`, which depends on both `application` and `infra`, builds again).

- [ ] **Step 8: Manual end-to-end verification with the real model**

Run a long game session against a real model to confirm condensation actually triggers and the narration stays coherent afterward:

```bash
NARRATEUR_MODELE=./modeles/qwen2.5-3b-instruct-q4_k_m.gguf cargo run --release -p app
```

Play enough turns (free-text exploration turns are fine, combat not required) that `etat.resumes`'s total word count should cross `SEUIL_MOTS_RESUMES` (800) — roughly 15-20 turns, since each turn's narration is a few short sentences. Between turns, inspect `partie.json` (`cat partie.json | python3 -m json.tool` or similar) to watch the `resumes` array: it should grow turn by turn, then at some point collapse from many entries down to `1 + NB_RESUMES_RECENTS_CONSERVES` (6) entries, with the first one being a condensed paragraph covering the earlier turns. Confirm the game remains playable and the narration doesn't contradict the condensed summary in later turns.

- [ ] **Step 9: Commit**

```bash
git add crates/infra/src/prompts.rs crates/infra/src/faux_narrateur.rs crates/infra/src/llama_narrateur.rs
git commit -m "feat(infra): implement résumé condensation prompt and both Narrateur implementors"
```
