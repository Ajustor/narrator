# LLM Narrateur Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the need for `FauxNarrateur` as the only `Narrateur` implementation with a real local LLM (via `llama-cpp-2`) that interprets the player's free text into a structured action (grammar-constrained JSON generation), then narrates the real mechanical outcome once dice are resolved — never the reverse.

**Architecture:** Split the `Narrateur` port into two calls (`proposer_action`, `raconter_resultat`) so dice resolution can happen *between* them. Everything pure (JSON Schema construction, prompt construction, JSON→`ActionJoueur` parsing) is extracted into its own TDD-able functions with zero dependency on the model. Only the final `LlamaNarrateur` struct touches `llama-cpp-2` directly, and it is not unit-testable (no automated test calls a real model) — only `cargo build` verifies it compiles; a human verifies it works by actually playing with a downloaded model afterward.

**Tech Stack:** Rust, `llama-cpp-2` (v0.1.160 verified maintained on crates.io as of 2026-10-15, by `utilityai`), `serde_json` (already a dependency), `encoding_rs` (new — UTF-8 streaming decode of token bytes).

**Spec:** `docs/superpowers/specs/2026-10-24-llm-narrateur-design.md`

## Global Constraints

- `llama-cpp-2` is added to `crates/infra/Cargo.toml` only — `domain` and `application` stay independent of the inference engine; only the `Narrateur` port concerns them, its concrete implementation lives in `infra`.
- `domain` depends on nothing external except `serde`. This plan does not touch `domain` at all.
- The `Narrateur` port's `proposer_action` must never resolve dice, apply damage, or decide any numeric outcome — only `raconter_resultat` (called *after* the engine has already resolved everything) narrates, and it narrates the real result, never an invented one.
- Enemies are never resolved via the `Narrateur`/LLM port — unchanged from the prior plan. `resoudre_action_ennemis` takes no `Narrateur` parameter and this plan does not change that.
- Model file path comes from the `NARRATEUR_MODELE` environment variable. A missing or unloadable model is a clear error message + non-zero exit — never a silent fallback to `FauxNarrateur`.
- Recommended model for manual verification after this plan: Qwen2.5-3B-Instruct, GGUF, Q4_K_M quantization (~2 GB) — this is the human's job to download and test with, not part of any task's automated verification.
- `FauxNarrateur` is kept in the codebase (updated to the new two-method port shape) as a test double — it is not deleted.

---

## Task 1: Verify `llama-cpp-2` compiles

**Files:**
- Modify: `crates/infra/Cargo.toml`

**Interfaces:**
- Produces: nothing code-visible — this task's only deliverable is confirming `cargo build -p infra` succeeds with `llama-cpp-2` as a dependency. `llama-cpp-2` compiles a vendored C++ core (llama.cpp) via a build script, which requires `cmake` and a C/C++ compiler on the system — this task is a deliberate early risk gate, isolated so a build failure here is diagnosed before any other task is attempted.

- [ ] **Step 1: Add the dependency**

Add this line to the `[dependencies]` section of `crates/infra/Cargo.toml` (keep the existing `domain`, `application`, `serde_json`, `rand` lines unchanged):

```toml
llama-cpp-2 = "0.1.160"
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p infra`

Expected: compiles successfully (this will take noticeably longer than previous builds — it's compiling llama.cpp's C++ core via a build script; a few minutes is normal, not a hang).

**If this fails:** read the actual error. If it's a missing system tool (`cmake: command not found`, a missing C/C++ compiler, or similar), report back with status **BLOCKED** and the exact error — do not attempt to work around it by removing the dependency or guessing at feature flags. The controller needs this information to decide whether to install the missing tool or reconsider the inference crate choice (the project's own planning notes flag `candle` as a fallback if `llama.cpp` compilation blocks). If it's some other kind of error (version resolution conflict, etc.), include the full error text in your report either way.

- [ ] **Step 3: Commit**

```bash
git add crates/infra/Cargo.toml
git commit -m "chore(infra): add llama-cpp-2 dependency, verify it builds"
```

---

## Task 2: Split the `Narrateur` port and update its consumers

**Files:**
- Modify: `crates/application/src/ports/narrateur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`
- Modify: `crates/infra/src/faux_narrateur.rs`

**Interfaces:**
- Consumes: `domain::{ActionJoueur, EtatPartie}` (unchanged).
- Produces: `application::ports::narrateur::{ContexteTour, ContexteResultat, ErreurNarrateur, Narrateur}` where `Narrateur` now has two methods: `proposer_action(&mut self, contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur>` and `raconter_resultat(&mut self, contexte: &ContexteResultat, sur_fragment: &mut dyn FnMut(&str)) -> Result<String, ErreurNarrateur>`. `ContexteResultat { etat: EtatPartie, action: ActionJoueur, resume_mecanique: Vec<String> }`. `ReponseMj` is removed (no longer needed — its two fields are now split across the two method return types).
- `application::usecases::jouer_tour::{JouerTour, ResultatTour, ErreurTour, valider_action, resoudre_action_joueur, resoudre_action_ennemis}` keep their existing signatures — only `JouerTour::executer`'s internal orchestration changes (it now calls `proposer_action` before resolution and `raconter_resultat` after, instead of a single `raconter` call before resolution).
- `infra::FauxNarrateur` keeps its existing `pub struct FauxNarrateur;` shape, now implementing the two-method trait.

This task touches three files that must change together and compile together — the trait, its only current implementation, and its only current consumer cannot be split into independent tasks without leaving the crate in a non-compiling state between them.

- [ ] **Step 1: Rewrite the port**

Replace the full contents of `crates/application/src/ports/narrateur.rs`:

```rust
use domain::{ActionJoueur, EtatPartie};

#[derive(Debug, Clone)]
pub struct ContexteTour {
    pub etat: EtatPartie,
    pub saisie_joueur: String,
}

#[derive(Debug, Clone)]
pub struct ContexteResultat {
    pub etat: EtatPartie,
    pub action: ActionJoueur,
    pub resume_mecanique: Vec<String>,
}

#[derive(Debug)]
pub enum ErreurNarrateur {
    EchecGeneration(String),
}

/// Contract for any implementation of this port, including the real
/// (LLM-backed) one in `infra::LlamaNarrateur` — `infra::FauxNarrateur` is
/// the test double.
///
/// 1. `proposer_action` must NEVER resolve dice, apply damage, or decide any
///    numeric game outcome — it only proposes an `ActionJoueur` interpreted
///    from the player's free text. `application`'s use cases validate and
///    resolve everything mechanical afterward, and only then is
///    `raconter_resultat` called with the real, already-resolved outcome.
/// 2. `raconter_resultat`'s `sur_fragment` callback must be invoked with
///    pieces of text that, concatenated in call order, reconstruct the
///    returned narration string exactly.
/// 3. A real LLM-backed implementation is responsible for extracting/parsing
///    the proposed action from whatever structured format the model
///    outputs (e.g. a JSON block) — that parsing is the implementation's
///    responsibility, not something `application` does for it.
pub trait Narrateur: Send {
    /// Proposes an action for the player's free-text input. No narration,
    /// no streaming — this result is never shown to the player directly.
    fn proposer_action(
        &mut self,
        contexte: &ContexteTour,
    ) -> Result<ActionJoueur, ErreurNarrateur>;

    /// Narrates the real outcome of an action already resolved by the
    /// engine (see `ContexteResultat`). Streamed to the player via
    /// `sur_fragment`.
    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur>;
}
```

- [ ] **Step 2: Update `FauxNarrateur` to implement the new port shape**

Replace the full contents of `crates/infra/src/faux_narrateur.rs`:

```rust
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
use domain::{ActionJoueur, EffetObjet};

pub struct FauxNarrateur;

impl Narrateur for FauxNarrateur {
    fn proposer_action(
        &mut self,
        contexte: &ContexteTour,
    ) -> Result<ActionJoueur, ErreurNarrateur> {
        let saisie_minuscule = contexte.saisie_joueur.to_lowercase();

        let action = if saisie_minuscule.contains("attaque") || saisie_minuscule.contains("frappe") {
            trouver_premier_ennemi_vivant(contexte)
                .map(|nom| ActionJoueur::Attaquer { cible: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre(contexte.saisie_joueur.clone()))
        } else if saisie_minuscule.contains("potion") || saisie_minuscule.contains("soin") {
            trouver_premiere_potion(contexte)
                .map(|nom| ActionJoueur::UtiliserObjet { objet: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre(contexte.saisie_joueur.clone()))
        } else if saisie_minuscule.contains("fui") || saisie_minuscule.contains("fuite") {
            ActionJoueur::Fuir
        } else {
            ActionJoueur::ActionLibre(contexte.saisie_joueur.clone())
        };

        Ok(action)
    }

    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let resultat = if contexte.resume_mecanique.is_empty() {
            "Rien de notable ne se produit.".to_string()
        } else {
            contexte.resume_mecanique.join(" ")
        };
        let narration = format!("Tu as tenté quelque chose... {}", resultat);
        for mot in narration.split_inclusive(' ') {
            sur_fragment(mot);
        }
        Ok(narration)
    }
}

fn trouver_premier_ennemi_vivant(contexte: &ContexteTour) -> Option<String> {
    contexte
        .etat
        .combat_en_cours
        .as_ref()?
        .ennemis
        .iter()
        .find(|e| !e.est_mort())
        .map(|e| e.nom.clone())
}

fn trouver_premiere_potion(contexte: &ContexteTour) -> Option<String> {
    contexte
        .etat
        .personnage
        .inventaire
        .iter()
        .find(|o| matches!(o.effet, Some(EffetObjet::Consommable { .. })))
        .map(|o| o.nom.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, EtatPartie, Objet, Personnage};

    fn contexte(saisie: &str, combat: Option<Combat>) -> ContexteTour {
        ContexteTour {
            etat: EtatPartie {
                personnage: Personnage::nouveau(
                    "Elowen".into(),
                    "Guerrier".into(),
                    Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                    vec![Objet::potion_de_soin()],
                ),
                lieu_actuel: "Clairière".into(),
                combat_en_cours: combat,
                resumes: vec![],
            },
            saisie_joueur: saisie.to_string(),
        }
    }

    fn combat_avec_gobelin() -> Combat {
        Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        }
    }

    #[test]
    fn attaque_propose_d_attaquer_le_premier_ennemi_vivant() {
        let ctx = contexte("J'attaque le gobelin", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn potion_propose_d_utiliser_la_potion_en_inventaire() {
        let ctx = contexte("Je bois une potion de soin", None);
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn fuite_propose_de_fuir() {
        let ctx = contexte("Je fuis !", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::Fuir);
    }

    #[test]
    fn texte_sans_mot_cle_devient_une_action_libre() {
        let ctx = contexte("J'observe les environs", None);
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::ActionLibre("J'observe les environs".into()));
    }

    #[test]
    fn la_narration_du_resultat_est_streamee_fragment_par_fragment() {
        let ctx = contexte("peu importe", None);
        let ctx_resultat = ContexteResultat {
            etat: ctx.etat,
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let mut narrateur = FauxNarrateur;
        let mut fragments = Vec::new();
        let narration = narrateur
            .raconter_resultat(&ctx_resultat, &mut |frag| fragments.push(frag.to_string()))
            .unwrap();
        assert_eq!(fragments.concat(), narration);
        assert!(fragments.len() > 1);
    }

    #[test]
    fn la_narration_du_resultat_reprend_le_resume_mecanique() {
        let ctx = contexte("peu importe", None);
        let ctx_resultat = ContexteResultat {
            etat: ctx.etat,
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            resume_mecanique: vec!["🎲 Tu attaques Gobelin : touché ! Dégâts : 5".to_string()],
        };
        let mut narrateur = FauxNarrateur;
        let narration = narrateur.raconter_resultat(&ctx_resultat, &mut |_| {}).unwrap();
        assert!(narration.contains("touché"));
    }
}
```

- [ ] **Step 3: Run the crates that no longer compile, confirm the expected failures**

Run: `cargo build --workspace 2>&1 | head -40`
Expected: FAIL — `crates/application/src/usecases/jouer_tour.rs` still calls the old `narrateur.raconter(...)` and imports `ReponseMj`, which no longer exist. This confirms Step 1-2 actually changed the port (not a silent no-op) before you fix the one remaining consumer in Step 4.

- [ ] **Step 4: Update `JouerTour::executer`'s orchestration**

In `crates/application/src/usecases/jouer_tour.rs`:

Replace the import line at the top:
```rust
use crate::ports::narrateur::{ContexteTour, Narrateur, ReponseMj};
```
with:
```rust
use crate::ports::narrateur::{ContexteResultat, ContexteTour, Narrateur};
```

Replace the entire body of `impl JouerTour { pub fn executer(...) { ... } }` (keep the `pub struct JouerTour;` line and the function signature unchanged, only the body changes):

```rust
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        let contexte_tour = ContexteTour {
            etat: etat.clone(),
            saisie_joueur: saisie.to_string(),
        };
        let action_proposee = narrateur
            .proposer_action(&contexte_tour)
            .map_err(|_| ErreurTour::EchecNarrateur)?;

        valider_action(&action_proposee, etat)?;

        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.tour_actuel += 1;
        }

        let mut resume = Vec::new();
        let fuite = matches!(action_proposee, ActionJoueur::Fuir);

        if fuite {
            resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
        } else {
            let ennemi_avant = etat
                .combat_en_cours
                .as_ref()
                .map(|c| c.ennemi_agit_avant_joueur)
                .unwrap_or(false);

            if ennemi_avant {
                resume.extend(resoudre_action_ennemis(etat, de));
                terminer_si_joueur_mort(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
                    terminer_si_ennemis_morts(etat);
                }
            } else {
                resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
                terminer_si_ennemis_morts(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_ennemis(etat, de));
                    terminer_si_joueur_mort(etat);
                }
            }
        }

        let contexte_resultat = ContexteResultat {
            etat: etat.clone(),
            action: action_proposee,
            resume_mecanique: resume.clone(),
        };
        let narration = narrateur
            .raconter_resultat(&contexte_resultat, sur_fragment)
            .map_err(|_| ErreurTour::EchecNarrateur)?;

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            combat_termine: etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(false),
        })
    }
```

Note the ordering change from the previous version: narration is now requested *after* the full round's mechanics resolve (both the player's and, if applicable, the enemy's), not before. This is deliberate: if an enemy acting first kills the player before the player's own action ever resolves, there is still a complete, coherent `resume_mecanique` to narrate; narrating before resolution would leave nothing truthful to say about an action that never happened.

- [ ] **Step 5: Update the test module's `NarrateurDeTest` double**

In `crates/application/src/usecases/jouer_tour.rs`'s `#[cfg(test)] mod tests` block, replace this import line:
```rust
    use crate::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};
```
with:
```rust
    use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
```

Replace the `NarrateurDeTest` struct and its `impl Narrateur` block:
```rust
    struct NarrateurDeTest {
        action: ActionJoueur,
    }

    impl Narrateur for NarrateurDeTest {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(self.action.clone())
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            sur_fragment("narration de test");
            Ok("narration de test".to_string())
        }
    }
```

The four existing tests that use `NarrateurDeTest` (`jouer_tour_joueur_tue_l_ennemi_et_termine_le_combat`, `jouer_tour_ennemi_plus_rapide_agit_avant_le_joueur`, `jouer_tour_hors_combat_ne_signale_jamais_combat_termine`, `jouer_tour_rejette_une_action_invalide_sans_consommer_de_de`) call `JouerTour::executer(...)` the same way as before — only the double's internals changed, so their bodies need no edits.

- [ ] **Step 6: Write a new failing test proving narration happens after resolution**

Add this new test to the same `mod tests` block (place it near the other `jouer_tour_*` tests):

```rust
    use std::cell::RefCell;

    struct NarrateurEspion {
        action: ActionJoueur,
        contexte_resultat_recu: RefCell<Option<ContexteResultat>>,
    }

    impl Narrateur for NarrateurEspion {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(self.action.clone())
        }

        fn raconter_resultat(
            &mut self,
            contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            *self.contexte_resultat_recu.borrow_mut() = Some(contexte.clone());
            Ok(String::new())
        }
    }

    #[test]
    fn jouer_tour_informe_la_narration_avec_le_resultat_mecanique_reel() {
        let mut etat = etat_en_combat();
        let mut narrateur = NarrateurEspion {
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            contexte_resultat_recu: RefCell::new(None),
        };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3 (mains nues)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de, &mut |_| {}).unwrap();

        let contexte_recu = narrateur.contexte_resultat_recu.borrow();
        let contexte_recu = contexte_recu.as_ref().expect("raconter_resultat aurait dû être appelé");
        assert!(contexte_recu.resume_mecanique.iter().any(|l| l.contains("touché")));
        assert_eq!(contexte_recu.action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }
```

- [ ] **Step 7: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS — all prior tests (adjusted in Steps 2 and 5) plus the new test from Step 6. The total count will be 65 (prior) minus the 5 old `FauxNarrateur` tests restructured into 6 (one split into two: the old single `raconter`-based streaming test became two tests — a pure streaming test and a resume-mecanique-content test) plus 1 new `jouer_tour` test — net +2 from 65.

- [ ] **Step 8: Commit**

```bash
git add crates/application/src/ports/narrateur.rs crates/application/src/usecases/jouer_tour.rs crates/infra/src/faux_narrateur.rs
git commit -m "feat: split Narrateur port into proposer_action/raconter_resultat so dice resolve before narration"
```

---

## Task 3: Dynamic JSON Schema builder for action classification

**Files:**
- Create: `crates/infra/src/action_schema.rs`
- Modify: `crates/infra/src/lib.rs`

**Interfaces:**
- Consumes: `domain::{EffetObjet, EtatPartie}`.
- Produces: `infra::action_schema::construire_schema_action(etat: &EtatPartie) -> serde_json::Value` — a JSON Schema (as a `serde_json::Value`) whose `cible`/`objet` enums are built from the *current* game state (living enemy names, consumable item names), and whose `action` enum only lists variants that are actually legal right now (`attaquer` only if an enemy is alive, `utiliser_objet` only if a consumable is in inventory, `fuir` only if combat is active, `libre` always). This task does not touch anything that calls `json_schema_to_grammar` — that wiring happens in Task 6.

- [ ] **Step 1: Write the failing tests**

```rust
use domain::{EffetObjet, EtatPartie};
use serde_json::Value;

pub fn construire_schema_action(etat: &EtatPartie) -> Value {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

    fn personnage_test(inventaire: Vec<Objet>) -> Personnage {
        Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
            inventaire,
        )
    }

    fn etat_sans_rien() -> EtatPartie {
        EtatPartie {
            personnage: personnage_test(vec![]),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
        }
    }

    #[test]
    fn sans_combat_ni_objet_seule_l_action_libre_est_possible() {
        let schema = construire_schema_action(&etat_sans_rien());
        let variantes = schema["oneOf"].as_array().unwrap();
        assert_eq!(variantes.len(), 1);
        assert_eq!(variantes[0]["properties"]["action"]["const"], "libre");
    }

    #[test]
    fn un_ennemi_vivant_ajoute_la_variante_attaquer_avec_son_nom() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        let attaquer = variantes
            .iter()
            .find(|v| v["properties"]["action"]["const"] == "attaquer")
            .expect("la variante attaquer doit être présente");
        let cibles = attaquer["properties"]["cible"]["enum"].as_array().unwrap();
        assert_eq!(cibles, &vec![Value::String("Gobelin".to_string())]);
    }

    #[test]
    fn un_ennemi_mort_n_apparait_pas_dans_les_cibles() {
        let mut etat = etat_sans_rien();
        let mut gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        gobelin.subir_degats(100);
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![gobelin],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "attaquer"));
    }

    #[test]
    fn un_objet_consommable_ajoute_la_variante_utiliser_objet() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::potion_de_soin()]);
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        let utiliser = variantes
            .iter()
            .find(|v| v["properties"]["action"]["const"] == "utiliser_objet")
            .expect("la variante utiliser_objet doit être présente");
        let objets = utiliser["properties"]["objet"]["enum"].as_array().unwrap();
        assert_eq!(objets, &vec![Value::String("Potion de soin".to_string())]);
    }

    #[test]
    fn une_arme_n_apparait_pas_dans_les_objets_utilisables() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::epee()]);
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "utiliser_objet"));
    }

    #[test]
    fn un_combat_actif_ajoute_la_variante_fuir() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().any(|v| v["properties"]["action"]["const"] == "fuir"));
    }

    #[test]
    fn un_combat_termine_n_ajoute_pas_la_variante_fuir() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: true,
            vainqueur: Some(domain::Vainqueur::Joueur),
        });
        let schema = construire_schema_action(&etat);
        let variantes = schema["oneOf"].as_array().unwrap();
        assert!(variantes.iter().all(|v| v["properties"]["action"]["const"] != "fuir"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra action_schema::tests`
Expected: FAIL — panics on `todo!()`.

- [ ] **Step 3: Write the implementation**

Replace the `todo!()` body:

```rust
pub fn construire_schema_action(etat: &EtatPartie) -> Value {
    let ennemis_vivants: Vec<Value> = etat
        .combat_en_cours
        .as_ref()
        .filter(|c| !c.termine)
        .map(|c| {
            c.ennemis
                .iter()
                .filter(|e| !e.est_mort())
                .map(|e| Value::String(e.nom.clone()))
                .collect()
        })
        .unwrap_or_default();

    let objets_consommables: Vec<Value> = etat
        .personnage
        .inventaire
        .iter()
        .filter(|o| matches!(o.effet, Some(EffetObjet::Consommable { .. })))
        .map(|o| Value::String(o.nom.clone()))
        .collect();

    let combat_actif = etat.combat_en_cours.as_ref().map(|c| !c.termine).unwrap_or(false);

    let mut variantes = vec![serde_json::json!({
        "type": "object",
        "properties": {
            "action": {"const": "libre"},
            "texte": {"type": "string"}
        },
        "required": ["action", "texte"]
    })];

    if !ennemis_vivants.is_empty() {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "attaquer"},
                "cible": {"enum": ennemis_vivants}
            },
            "required": ["action", "cible"]
        }));
    }

    if !objets_consommables.is_empty() {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "utiliser_objet"},
                "objet": {"enum": objets_consommables}
            },
            "required": ["action", "objet"]
        }));
    }

    if combat_actif {
        variantes.push(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {"const": "fuir"}
            },
            "required": ["action"]
        }));
    }

    serde_json::json!({ "oneOf": variantes })
}
```

- [ ] **Step 4: Wire the module**

Update `crates/infra/src/lib.rs` to add `mod action_schema;` alongside the existing modules (don't remove `faux_narrateur`, `generateur_de_os`, `sauvegarde_fichier` or their `pub use` lines):

```rust
mod action_schema;
mod faux_narrateur;
mod generateur_de_os;
mod sauvegarde_fichier;

pub use faux_narrateur::FauxNarrateur;
pub use generateur_de_os::GenerateurDeOs;
pub use sauvegarde_fichier::SauvegardeFichier;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (7 new tests, plus all prior `infra` tests still passing).

- [ ] **Step 6: Commit**

```bash
git add crates/infra/src/action_schema.rs crates/infra/src/lib.rs
git commit -m "feat(infra): add dynamic JSON Schema builder for action classification"
```

---

## Task 4: Prompt builders

**Files:**
- Create: `crates/infra/src/prompts.rs`
- Modify: `crates/infra/src/lib.rs`

**Interfaces:**
- Consumes: `application::ports::narrateur::{ContexteResultat, ContexteTour}`, `domain::EtatPartie`.
- Produces: `infra::prompts::{construire_prompt_classification(contexte: &ContexteTour) -> String, construire_prompt_narration(contexte: &ContexteResultat) -> String}`.

- [ ] **Step 1: Write the failing tests**

```rust
use application::ports::narrateur::{ContexteResultat, ContexteTour};
use domain::EtatPartie;

pub fn construire_prompt_classification(contexte: &ContexteTour) -> String {
    todo!()
}

pub fn construire_prompt_narration(contexte: &ContexteResultat) -> String {
    todo!()
}

fn description_combat(etat: &EtatPartie) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ActionJoueur, Caracteristiques, Combat, De, Ennemi, Personnage};

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                vec![],
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
        }
    }

    fn etat_en_combat() -> EtatPartie {
        let mut etat = etat_sans_combat();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        etat
    }

    #[test]
    fn le_prompt_de_classification_contient_la_saisie_du_joueur() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "Je regarde autour de moi".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Je regarde autour de moi"));
    }

    #[test]
    fn le_prompt_de_classification_mentionne_les_ennemis_en_combat() {
        let contexte = ContexteTour {
            etat: etat_en_combat(),
            saisie_joueur: "J'attaque".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Gobelin"));
    }

    #[test]
    fn le_prompt_de_classification_ne_mentionne_pas_de_combat_hors_combat() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(!prompt.contains("combat"));
    }

    #[test]
    fn le_prompt_de_narration_contient_le_resume_mecanique() {
        let contexte = ContexteResultat {
            etat: etat_en_combat(),
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            resume_mecanique: vec!["🎲 Tu attaques Gobelin : touché ! Dégâts : 5".to_string()],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("touché"));
        assert!(prompt.contains("Dégâts : 5"));
    }

    #[test]
    fn le_prompt_de_narration_gere_un_resume_vide_sans_paniquer() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra prompts::tests`
Expected: FAIL — panics on `todo!()`.

- [ ] **Step 3: Write the implementation**

Replace the three `todo!()` bodies:

```rust
pub fn construire_prompt_classification(contexte: &ContexteTour) -> String {
    format!(
        "Tu es le maître du jeu d'un jeu de rôle solo en français.\n\
         Le joueur se trouve à : {}.\n\
         {}\
         Le joueur dit : \"{}\"\n\
         Réponds uniquement par un objet JSON décrivant l'action correspondante, sans aucun autre texte.",
        contexte.etat.lieu_actuel,
        description_combat(&contexte.etat),
        contexte.saisie_joueur,
    )
}

pub fn construire_prompt_narration(contexte: &ContexteResultat) -> String {
    let resultat = if contexte.resume_mecanique.is_empty() {
        "Rien de mécanique ne s'est produit.".to_string()
    } else {
        contexte.resume_mecanique.join(" ")
    };
    format!(
        "Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        contexte.action, resultat,
    )
}

fn description_combat(etat: &EtatPartie) -> String {
    match &etat.combat_en_cours {
        Some(combat) if !combat.termine => {
            let noms: Vec<&str> = combat
                .ennemis
                .iter()
                .filter(|e| !e.est_mort())
                .map(|e| e.nom.as_str())
                .collect();
            if noms.is_empty() {
                String::new()
            } else {
                format!("Il est en combat contre : {}.\n", noms.join(", "))
            }
        }
        _ => String::new(),
    }
}
```

- [ ] **Step 4: Wire the module**

Update `crates/infra/src/lib.rs` to add `mod prompts;` alongside the existing modules:

```rust
mod action_schema;
mod faux_narrateur;
mod generateur_de_os;
mod prompts;
mod sauvegarde_fichier;

pub use faux_narrateur::FauxNarrateur;
pub use generateur_de_os::GenerateurDeOs;
pub use sauvegarde_fichier::SauvegardeFichier;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (5 new tests, plus all prior `infra` tests).

- [ ] **Step 6: Commit**

```bash
git add crates/infra/src/prompts.rs crates/infra/src/lib.rs
git commit -m "feat(infra): add prompt builders for classification and narration"
```

---

## Task 5: JSON → `ActionJoueur` parser

**Files:**
- Modify: `crates/infra/src/action_schema.rs`

**Interfaces:**
- Consumes: `domain::ActionJoueur`.
- Produces: `infra::action_schema::{interpreter_json_action(json_brut: &str) -> Result<ActionJoueur, ErreurInterpretation>, ErreurInterpretation}`. `ErreurInterpretation` is a simple newtype wrapping a `String` message — used by `LlamaNarrateur` in Task 6 to build an `ErreurNarrateur::EchecGeneration`.

This task appends to the file Task 3 created — it does not touch `construire_schema_action` or its tests.

- [ ] **Step 1: Write the failing tests**

Append to `crates/infra/src/action_schema.rs`, above the existing `#[cfg(test)] mod tests` block:

```rust
#[derive(Debug, PartialEq)]
pub struct ErreurInterpretation(pub String);

pub fn interpreter_json_action(json_brut: &str) -> Result<domain::ActionJoueur, ErreurInterpretation> {
    todo!()
}
```

Add these tests inside the existing `mod tests` block (the file already has `use super::*;` and `use domain::{...}` there — these tests only need `domain::ActionJoueur` added to that existing `use` line):

```rust
    #[test]
    fn interprete_une_attaque() {
        let action = interpreter_json_action(r#"{"action":"attaquer","cible":"Gobelin"}"#).unwrap();
        assert_eq!(action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn interprete_l_utilisation_d_un_objet() {
        let action = interpreter_json_action(r#"{"action":"utiliser_objet","objet":"Potion de soin"}"#).unwrap();
        assert_eq!(action, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn interprete_une_fuite() {
        let action = interpreter_json_action(r#"{"action":"fuir"}"#).unwrap();
        assert_eq!(action, ActionJoueur::Fuir);
    }

    #[test]
    fn interprete_une_action_libre() {
        let action = interpreter_json_action(r#"{"action":"libre","texte":"J'observe"}"#).unwrap();
        assert_eq!(action, ActionJoueur::ActionLibre("J'observe".into()));
    }

    #[test]
    fn un_json_invalide_echoue() {
        let resultat = interpreter_json_action("ceci n'est pas du json");
        assert!(resultat.is_err());
    }

    #[test]
    fn un_champ_action_manquant_echoue() {
        let resultat = interpreter_json_action(r#"{"cible":"Gobelin"}"#);
        assert!(resultat.is_err());
    }

    #[test]
    fn une_attaque_sans_cible_echoue() {
        let resultat = interpreter_json_action(r#"{"action":"attaquer"}"#);
        assert!(resultat.is_err());
    }

    #[test]
    fn une_action_inconnue_echoue() {
        let resultat = interpreter_json_action(r#"{"action":"voler"}"#);
        assert!(resultat.is_err());
    }
```

Update the existing `use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};` line in `mod tests` to also import `ActionJoueur`:
```rust
    use domain::{ActionJoueur, Caracteristiques, Combat, De, Ennemi, Objet, Personnage};
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra action_schema::tests`
Expected: FAIL — panics on `todo!()`.

- [ ] **Step 3: Write the implementation**

Replace the `todo!()` body:

```rust
pub fn interpreter_json_action(json_brut: &str) -> Result<domain::ActionJoueur, ErreurInterpretation> {
    use domain::ActionJoueur;

    let valeur: Value = serde_json::from_str(json_brut)
        .map_err(|e| ErreurInterpretation(format!("JSON invalide : {e}")))?;

    let action = valeur
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| ErreurInterpretation("champ \"action\" manquant".to_string()))?;

    match action {
        "attaquer" => {
            let cible = valeur
                .get("cible")
                .and_then(Value::as_str)
                .ok_or_else(|| ErreurInterpretation("champ \"cible\" manquant".to_string()))?;
            Ok(ActionJoueur::Attaquer { cible: cible.to_string() })
        }
        "utiliser_objet" => {
            let objet = valeur
                .get("objet")
                .and_then(Value::as_str)
                .ok_or_else(|| ErreurInterpretation("champ \"objet\" manquant".to_string()))?;
            Ok(ActionJoueur::UtiliserObjet { objet: objet.to_string() })
        }
        "fuir" => Ok(ActionJoueur::Fuir),
        "libre" => {
            let texte = valeur.get("texte").and_then(Value::as_str).unwrap_or("").to_string();
            Ok(ActionJoueur::ActionLibre(texte))
        }
        autre => Err(ErreurInterpretation(format!("action inconnue : {autre}"))),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (8 new tests, plus all prior `infra` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/infra/src/action_schema.rs
git commit -m "feat(infra): add JSON to ActionJoueur parser"
```

---

## Task 6: `LlamaNarrateur` — the real `llama-cpp-2` integration

**Files:**
- Create: `crates/infra/src/llama_narrateur.rs`
- Modify: `crates/infra/src/lib.rs`
- Modify: `crates/infra/Cargo.toml`

**Interfaces:**
- Consumes: `infra::action_schema::{construire_schema_action, interpreter_json_action}`, `infra::prompts::{construire_prompt_classification, construire_prompt_narration}`, `application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur}`, `domain::ActionJoueur`.
- Produces: `infra::LlamaNarrateur` with `LlamaNarrateur::charger(chemin_modele: &std::path::Path) -> Result<Self, String>`, implementing `Narrateur`.

**This task has no automated behavioral test** — there is no model file available to the implementer, and none should be downloaded as part of this task (that is the human's job, after this plan is complete, per the project's own decision). The acceptance bar for this task is **`cargo build -p infra` succeeding** — i.e., the code compiles and the types line up — not that it was run against a real model.

**Important — API verification required before writing code.** Part of this task's code (model loading, context creation, tokenization, the batch/decode loop) has been verified against `llama-cpp-2`'s own official example (`examples/simple/src/main.rs` in `github.com/utilityai/llama-cpp-rs`) and is given to you below as exact, working code. The grammar-constrained sampling part (`LlamaSampler::grammar(...)` and `llama_cpp_2::json_schema_to_grammar(...)`) was found through secondary sources, not the primary example, and may not match the exact signature in the version you have installed. **Before writing `generer`'s grammar-handling code, verify the real signatures** by running:

```bash
cargo doc -p llama-cpp-2 --no-deps --open
```

and reading the generated docs for `llama_cpp_2::json_schema_to_grammar` and `llama_cpp_2::sampling::LlamaSampler::grammar` (or search in your browser's find-in-page for "grammar" across the generated docs). If the real signature differs from what's below, adapt the code to match it — the behavioral contract that must be preserved is: convert the JSON Schema from `construire_schema_action` into a GBNF grammar string, then build a sampler that only emits tokens matching that grammar. Note in your self-review whether you had to adjust this part and how.

- [ ] **Step 1: Add the `encoding_rs` dependency**

Add this line to `crates/infra/Cargo.toml`'s `[dependencies]` section (alongside the existing `llama-cpp-2` line added in Task 1):

```toml
encoding_rs = "0.8"
```

- [ ] **Step 2: Verify the grammar API** (see the "Important" note above) before proceeding to Step 3.

- [ ] **Step 3: Write `LlamaNarrateur`**

Create `crates/infra/src/llama_narrateur.rs`:

```rust
use crate::action_schema::{construire_schema_action, interpreter_json_action};
use crate::prompts::{construire_prompt_classification, construire_prompt_narration};
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
use domain::ActionJoueur;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use std::num::NonZeroU32;
use std::path::Path;

const TAILLE_CONTEXTE: u32 = 4096;
const TOKENS_MAX_CLASSIFICATION: i32 = 100;
const TOKENS_MAX_NARRATION: i32 = 200;

pub struct LlamaNarrateur {
    backend: LlamaBackend,
    model: LlamaModel,
    ctx_params: LlamaContextParams,
}

impl LlamaNarrateur {
    pub fn charger(chemin_modele: &Path) -> Result<Self, String> {
        let backend = LlamaBackend::init()
            .map_err(|e| format!("échec d'initialisation du backend llama.cpp : {e}"))?;

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, chemin_modele, &model_params).map_err(|e| {
            format!("échec de chargement du modèle {} : {e}", chemin_modele.display())
        })?;

        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(TAILLE_CONTEXTE).unwrap()))
            .with_n_threads(threads)
            .with_n_threads_batch(threads);

        Ok(Self { backend, model, ctx_params })
    }

    fn generer(
        &self,
        prompt: &str,
        max_tokens: i32,
        grammaire: Option<&str>,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let mut ctx = self
            .model
            .new_context(&self.backend, self.ctx_params.clone())
            .map_err(|e| format!("échec de création du contexte : {e}"))?;

        let tokens_prompt = self.model.vocab().tokenize(prompt.as_bytes(), true, true);
        let mut batch = LlamaBatch::new(512, 1);
        let dernier = (tokens_prompt.len() - 1) as i32;
        for (i, token) in (0_i32..).zip(tokens_prompt.into_iter()) {
            batch
                .add(token, i, &[0], i == dernier)
                .map_err(|e| format!("échec d'ajout au batch : {e}"))?;
        }
        ctx.decode(&mut batch)
            .map_err(|e| format!("échec de décodage du prompt : {e}"))?;

        // NOTE: verify this block against the real API per Step 2 above.
        let grammaire_sampler = grammaire
            .map(|g| {
                let gbnf = llama_cpp_2::json_schema_to_grammar(g, false)
                    .map_err(|e| format!("conversion JSON Schema → grammaire échouée : {e}"))?;
                LlamaSampler::grammar(&self.model, &gbnf, "root")
                    .map_err(|e| format!("grammaire invalide : {e}"))
            })
            .transpose()?;

        let mut chaine = Vec::new();
        if let Some(s) = grammaire_sampler {
            chaine.push(s);
        }
        chaine.push(LlamaSampler::dist(1234));
        chaine.push(LlamaSampler::greedy());
        let mut sampler = LlamaSampler::chain_simple(chaine);

        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut sortie = String::new();
        let mut n_cur = batch.n_tokens();
        let n_len = n_cur + max_tokens;

        while n_cur <= n_len {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            if self.model.vocab().is_eog(token) {
                break;
            }

            let morceau = self.model.vocab().token_to_piece(token, true, None);
            let mut texte = String::with_capacity(
                decoder.max_utf8_buffer_length(morceau.len()).unwrap_or(morceau.len()),
            );
            let _ = decoder.decode_to_string(&morceau, &mut texte, false);
            sur_fragment(&texte);
            sortie.push_str(&texte);

            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| format!("échec d'ajout au batch : {e}"))?;
            n_cur += 1;
            ctx.decode(&mut batch).map_err(|e| format!("échec de décodage : {e}"))?;
        }

        Ok(sortie)
    }
}

impl Narrateur for LlamaNarrateur {
    fn proposer_action(&mut self, contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
        let schema = construire_schema_action(&contexte.etat);
        let prompt = construire_prompt_classification(contexte);
        let json_brut = self
            .generer(&prompt, TOKENS_MAX_CLASSIFICATION, Some(&schema.to_string()), &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)?;
        interpreter_json_action(&json_brut).map_err(|e| ErreurNarrateur::EchecGeneration(e.0))
    }

    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_narration(contexte);
        self.generer(&prompt, TOKENS_MAX_NARRATION, None, sur_fragment)
            .map_err(ErreurNarrateur::EchecGeneration)
    }
}
```

If `LlamaContextParams` does not implement `Clone` (needed for `self.ctx_params.clone()`, since `generer` is called twice — once for classification, once for narration — and `new_context` consumes its params argument by value), store `LlamaContextParams`'s constituent values (context size, thread count) as plain fields on `LlamaNarrateur` instead, and rebuild a fresh `LlamaContextParams` inside `generer` each time. Note in your self-review which approach you used.

- [ ] **Step 4: Wire the module**

Update `crates/infra/src/lib.rs` to add `mod llama_narrateur;` and `pub use llama_narrateur::LlamaNarrateur;` alongside the existing modules:

```rust
mod action_schema;
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

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p infra`
Expected: compiles successfully. This does NOT exercise `charger()` or `generer()` at runtime (no model file exists in this environment) — it only confirms the code is type-correct against the real `llama-cpp-2` API you verified in Step 2.

If it fails to compile because the grammar API (or any other API surface) differs from what's written above, fix the specific lines that don't match — the rest of the file (model loading, context, batch, decode loop) was verified against the crate's own official example and should not need changes.

- [ ] **Step 6: Commit**

```bash
git add crates/infra/src/llama_narrateur.rs crates/infra/src/lib.rs crates/infra/Cargo.toml
git commit -m "feat(infra): add LlamaNarrateur — real LLM integration via llama-cpp-2"
```

---

## Task 7: Wire `LlamaNarrateur` into the CLI

**Files:**
- Modify: `crates/app/src/main.rs`

**Interfaces:**
- Consumes: `infra::LlamaNarrateur`, everything else unchanged from the prior plan.
- Produces: a runnable binary that reads the `NARRATEUR_MODELE` environment variable, loads a real model, and plays using it — `FauxNarrateur` is no longer instantiated by `app` (it remains available in `infra` purely as a test double for `application`'s own test suite).

This task has no automated test — the resulting binary cannot be exercised without a real model file, which is explicitly the human's job after this plan (per the project's decision — see Global Constraints). The acceptance bar is `cargo build -p app` succeeding and the control flow being correct by inspection.

- [ ] **Step 1: Update `main.rs`**

Replace these two lines near the top of `crates/app/src/main.rs`:
```rust
use infra::{FauxNarrateur, GenerateurDeOs, SauvegardeFichier};
```
with:
```rust
use infra::{GenerateurDeOs, LlamaNarrateur, SauvegardeFichier};
```

Replace this line:
```rust
    let mut narrateur = FauxNarrateur;
```
with:
```rust
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        eprintln!("Exemple : NARRATEUR_MODELE=./modeles/qwen2.5-3b-instruct-q4_k_m.gguf cargo run -p app");
        std::process::exit(1);
    });
    let mut narrateur = LlamaNarrateur::charger(std::path::Path::new(&chemin_modele)).unwrap_or_else(|erreur| {
        eprintln!("Échec du chargement du modèle : {erreur}");
        std::process::exit(1);
    });
```

Nothing else in `main.rs` changes — `JouerTour::executer(&mut etat, saisie, &mut narrateur, &mut de, &mut |frag: &str| {...})` is called exactly the same way as before; the two-method split from Task 2 is internal to `JouerTour`/`Narrateur` and invisible to `main.rs`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p app`
Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/main.rs
git commit -m "feat(app): wire LlamaNarrateur into the CLI via NARRATEUR_MODELE"
```

---

## Completion check

After Task 7:

- `cargo test --workspace` passes with 0 failures (covers everything except `LlamaNarrateur` and `main.rs`'s new branch, which have no automated tests by design).
- `cargo build --workspace` and `cargo build --release` both succeed.
- **Manual verification (human, not part of this plan's automated execution):** download Qwen2.5-3B-Instruct in GGUF Q4_K_M format, then run `NARRATEUR_MODELE=<path-to-gguf> cargo run -p app` and play a few turns — confirm free-text input is interpreted into a sensible action, and that the narration reflects the real dice outcome (hit/miss, damage) rather than contradicting it.
