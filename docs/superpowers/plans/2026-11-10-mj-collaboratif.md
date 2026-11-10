# Un MJ qui travaille avec le joueur — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the unavoidable opening combat, let combat start dynamically
and judge player intent during it, teach the model the `*action*` vs speech
convention, and let the player set a genre + free-text premise that shapes
the whole story, not just its opening line.

**Architecture:** Pure additions in `domain` (new enum/struct types, one new
`ActionJoueur` variant, one existing variant reshaped), a cascade through
`application`/`infra` wherever those types are matched or constructed, and a
UI slice in `app_desktop` last, once the types it needs already exist. Each
task lands a complete, independently working slice (`cargo test --workspace`
green at the end of every task) rather than splitting along crate
boundaries, because Rust's exhaustiveness checking makes a type change and
its call-site fixes inseparable at compile time.

**Tech Stack:** Rust workspace (`domain`/`application`/`infra`/`app`/
`app_desktop`), serde, Dioxus 0.7.

**Spec:** `docs/superpowers/specs/2026-11-10-mj-collaboratif-design.md`

## Global Constraints

- Three enemy difficulty tiers, exact stats (nom/pv_max/degats/defense/
  bonus_initiative/bonus_attaque order matches `Ennemi::nouveau`):
  Faible = 4 PV, 1d4, défense 10, init +1, attaque +1.
  Moyen = 7 PV, 1d6, défense 12, init +2, attaque +3 (identical to the old
  hardcoded Gobelin — default difficulty must not feel harder than before).
  Fort = 12 PV, 1d8, défense 14, init +3, attaque +5.
- `ActionLibre`'s `agressif` field defaults to `false` on any classifier
  fallback/parse-failure path — never `true`. This is the one value in the
  whole plan where getting the default backwards silently reintroduces the
  bug being fixed.
- Classification prompt must instruct the model to prefer "faible" by
  default for `provoquer_combat`, reserving "moyen"/"fort" for situations
  where the player was clearly aggressive or the story already escalated.
- Premise free text is capped at 300 characters, enforced in two places:
  the UI `<textarea maxlength>` and again at prompt-build time
  (`description_premisse`) — never trust only one.
- Removing a `#[serde(default)]` field from `EtatPartie` is safe for old
  saves (serde ignores unknown fields on read). Adding a new *non-optional*
  field is only safe with `#[serde(default)]` plus a `Default` impl.
- Do not touch `construire_schema_action` or its tests in
  `crates/infra/src/action_schema.rs` — confirmed dead code
  (`#[allow(dead_code)]`, abandoned GBNF approach), not on the path that
  needs to change for any of this to work. Only `interpreter_json_action`
  in that same file is live.
- No bestiary, no conditions/saving throws, no spell slots — stays within
  the existing "D&D allégé" scope.

## Review Focus

1. Malformed or out-of-schema classifier JSON during an active combat (a
   known, routine failure mode with the current 3B local model) must never
   make `agressif` default to `true` — pinned by a dedicated fallback test
   in Task 2.
2. A brand-new game's very first action, when it isn't aggressive toward a
   named target, must leave `combat_en_cours` at `None` — pinned by a
   regression test in Task 4 (this is the exact anti-objectif from the
   spec).
3. The classifier proposing `provoquer_combat` while a combat is *already*
   active (a plausible misclassification) must not panic, duplicate, or
   silently drop the turn — pinned in Task 3.
4. An old save file written before this feature (no `premisse` field, and
   still carrying the now-removed `rencontre_initiale_declenchee`) must
   still load — pinned in Task 4 (field removal) and Task 6 (field
   addition with `#[serde(default)]`).
5. A premise longer than 300 characters, however it got stored, must be
   truncated at prompt-build time regardless of what the UI enforced —
   pinned in Task 6.

---

## Task 1: `Difficulte` and `gabarit_ennemi`

**Files:**
- Modify: `crates/domain/src/ennemi.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Produces: `pub enum Difficulte { Faible, Moyen, Fort }` (derive `Debug,
  Clone, Copy, PartialEq, Eq, Serialize, Deserialize`); `pub fn
  gabarit_ennemi(nom: String, difficulte: Difficulte) -> Ennemi`. Both
  exported from `domain` (added to the `ennemi` line in `lib.rs`).

This task is purely additive — nothing elsewhere references these yet, so
it is safe to land on its own.

- [ ] **Step 1: Write the failing tests** in `crates/domain/src/ennemi.rs`'s
  `#[cfg(test)]` module:

```rust
#[test]
fn gabarit_ennemi_faible_a_les_stats_du_palier_faible() {
    let e = gabarit_ennemi("Rat".into(), Difficulte::Faible);
    assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
        (4, De { faces: 4 }, 10, 1, 1));
}

#[test]
fn gabarit_ennemi_moyen_reprend_les_stats_de_l_ancien_gobelin() {
    let e = gabarit_ennemi("Gobelin".into(), Difficulte::Moyen);
    assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
        (7, De { faces: 6 }, 12, 2, 3));
}

#[test]
fn gabarit_ennemi_fort_a_les_stats_du_palier_fort() {
    let e = gabarit_ennemi("Ogre".into(), Difficulte::Fort);
    assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
        (12, De { faces: 8 }, 14, 3, 5));
}

#[test]
fn gabarit_ennemi_garde_le_nom_donne() {
    assert_eq!(gabarit_ennemi("Loup".into(), Difficulte::Faible).nom, "Loup");
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test -p domain ennemi::` —
  expected: fails to compile (`Difficulte`/`gabarit_ennemi` don't exist).

- [ ] **Step 3: Implement `Difficulte` and `gabarit_ennemi` in
  `crates/domain/src/ennemi.rs`.** Use the exact stat table from Global
  Constraints; `gabarit_ennemi` just calls `Ennemi::nouveau` with the
  matched tier's values.

- [ ] **Step 4: Export from `crates/domain/src/lib.rs`** — change
  `pub use ennemi::Ennemi;` to `pub use ennemi::{gabarit_ennemi, Difficulte, Ennemi};`.

- [ ] **Step 5: Run and verify pass** — `cargo test -p domain ennemi::` —
  expected: 4 new tests PASS, existing `Ennemi` tests still PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/domain/src/ennemi.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add enemy difficulty tiers and gabarit_ennemi"
```

---

## Task 2: Judge intent on free actions during combat

**Files:**
- Modify: `crates/domain/src/action_joueur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`
- Modify: `crates/infra/src/action_schema.rs` (only `interpreter_json_action`
  and its tests — not `construire_schema_action`, see Global Constraints)
- Modify: `crates/infra/src/prompts.rs`
- Modify: `crates/infra/src/llama_narrateur.rs`
- Modify: `crates/infra/src/faux_narrateur.rs`
- Modify: `crates/app_desktop/src/worker.rs` (two test-double call sites only)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `domain::ActionJoueur::ActionLibre { texte: String, agressif:
  bool }` (was `ActionLibre(String)`) — every later task that constructs or
  matches this variant uses the new struct-variant shape. Also produces
  `infra::action_schema::interpreter_ou_repli_libre(json_brut: &str,
  saisie_joueur: &str) -> domain::ActionJoueur` (see Step 7), now the only
  place `LlamaNarrateur::proposer_action` turns raw model JSON into an
  `ActionJoueur`.

### Full list of call sites that must change (verified by search — this is
the complete set, not a sample)

Tuple-constructor call sites (`ActionLibre("...".into())` →
`ActionLibre { texte: "...".into(), agressif: <bool> }`, pick `agressif`
per the step below):
- `crates/application/src/usecases/jouer_tour.rs` — 14 occurrences (lines
  ~603, 684, 697, 712, 858, 881, 902, 918, 940, 963, 1025, 1066, 1107, 1192
  at time of writing; search for `ActionLibre(` to find the current set).
- `crates/app_desktop/src/worker.rs` — 2 occurrences (~506, 679).
- `crates/infra/src/faux_narrateur.rs` — 5 occurrences (~16, 20, 24, 158,
  166).
- `crates/infra/src/action_schema.rs` — 2 occurrences (~117, 315).
- `crates/infra/src/prompts.rs` — 9 occurrences (~270, 291, 341, 382, 393,
  410, 428, 445).
- `crates/infra/src/llama_narrateur.rs` — 2 occurrences (~196, 203).

Destructuring match sites (`ActionJoueur::ActionLibre(texte) => ...` or
`let ActionJoueur::ActionLibre(texte) = ... else { ... }` → `ActionJoueur::
ActionLibre { texte, .. } =>` / `let ActionJoueur::ActionLibre { texte, .. }
= ...`):
- `crates/application/src/usecases/jouer_tour.rs:64` (in `valider_action`)
  and `:153` (in `resoudre_action_joueur`).
- `crates/infra/src/prompts.rs:155` (in `description_pnj_pertinents`).

For every existing test call site above, set `agressif: false` (none of
them are testing aggression — they're all pre-existing non-combat or
generic-content tests). New tests in this task (Step 1 below) are the only
ones that need `agressif: true`.

- [ ] **Step 1: Write the failing tests**, in
  `crates/application/src/usecases/jouer_tour.rs`'s test module:

```rust
#[test]
fn action_libre_agressive_en_combat_devient_une_attaque_improvisee() {
    let mut etat = etat_en_combat();
    let mut de = GenerateurDeSeed::nouveau(vec![15, 15, 4]); // désavantage garde 15, dégâts 4
    let resume = resoudre_action_joueur(
        &mut etat,
        &ActionJoueur::ActionLibre { texte: "je frappe avec une chaise".into(), agressif: true },
        &mut de,
    );
    assert!(resume.iter().any(|l| l.contains("Tentative improvisée")));
    assert!(etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels < 7);
}

#[test]
fn action_libre_non_agressive_en_combat_ne_lance_aucun_de() {
    let mut etat = etat_en_combat();
    let mut de = GenerateurDeSeed::nouveau(vec![]); // doit paniquer si un dé est lancé
    let resume = resoudre_action_joueur(
        &mut etat,
        &ActionJoueur::ActionLibre { texte: "je négocie avec le gobelin".into(), agressif: false },
        &mut de,
    );
    assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels, 7);
    assert!(resume.iter().any(|l| l.contains("non-agressive")));
}
```

In `crates/infra/src/action_schema.rs`'s test module:

```rust
#[test]
fn interprete_une_action_libre_agressive() {
    let action = interpreter_json_action(r#"{"action":"libre","texte":"je frappe","agressif":true}"#).unwrap();
    assert_eq!(action, ActionJoueur::ActionLibre { texte: "je frappe".into(), agressif: true });
}

#[test]
fn interprete_une_action_libre_sans_champ_agressif_defaut_a_false() {
    let action = interpreter_json_action(r#"{"action":"libre","texte":"j'observe"}"#).unwrap();
    assert_eq!(action, ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false });
}
```

`LlamaNarrateur::proposer_action` isn't unit-testable in isolation (it
calls into a loaded GGUF model) — so this task extracts the fallback logic
into a pure, testable function instead of leaving it inline. In
`crates/infra/src/action_schema.rs`'s test module:

```rust
#[test]
fn repli_sur_json_invalide_devient_une_action_libre_non_agressive() {
    let action = interpreter_ou_repli_libre("ceci n'est pas du json", "je tente autre chose");
    assert_eq!(action, ActionJoueur::ActionLibre { texte: "je tente autre chose".into(), agressif: false });
}

#[test]
fn repli_sur_action_hors_schema_devient_une_action_libre_non_agressive() {
    let action = interpreter_ou_repli_libre(r#"{"action":"preparer_sort"}"#, "je prépare une boule de feu");
    assert_eq!(action, ActionJoueur::ActionLibre { texte: "je prépare une boule de feu".into(), agressif: false });
}

#[test]
fn une_action_reconnue_n_est_pas_affectee_par_le_repli() {
    let action = interpreter_ou_repli_libre(r#"{"action":"fuir"}"#, "peu importe");
    assert_eq!(action, ActionJoueur::Fuir);
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test --workspace` —
  expected: compile errors everywhere `ActionLibre(_)` is still a tuple
  pattern.

- [ ] **Step 3: Change the variant** in
  `crates/domain/src/action_joueur.rs`:

```rust
ActionLibre { texte: String, agressif: bool },
```

- [ ] **Step 4: Fix every call site** listed above. For the two
  destructuring sites in `jouer_tour.rs` (`valider_action` keeps `_ => Ok(())`
  equivalent: `ActionJoueur::ActionLibre { .. } => Ok(())`) and the
  `resoudre_action_joueur` arm, and the one in `prompts.rs`
  (`description_pnj_pertinents`, which only needs `texte`), use `{ texte, .. }`.

- [ ] **Step 5: Implement the branching in `resoudre_action_joueur`'s
  `ActionLibre` arm** (`jouer_tour.rs`): keep the existing attack-roll logic
  under `combat_actif && agressif`; add a new branch for `combat_actif &&
  !agressif` that pushes a message such as `format!("💬 Tentative
  non-agressive : « {texte} ».")` and does not call `de` at all; leave the
  non-combat branch unchanged (ignores `agressif`).

- [ ] **Step 6: Update `interpreter_json_action`'s `"libre"` arm**
  (`action_schema.rs`) to also read `valeur.get("agressif").and_then(Value::as_bool).unwrap_or(false)`
  and construct the struct variant.

- [ ] **Step 7: Extract the fallback into a pure, testable function.** Add
  `pub fn interpreter_ou_repli_libre(json_brut: &str, saisie_joueur: &str)
  -> domain::ActionJoueur` to `crates/infra/src/action_schema.rs`. It must
  reproduce exactly what `LlamaNarrateur::proposer_action` does today
  (parse-or-default, *then* always overwrite a `"libre"` result's `texte`
  with the verbatim player input — the model is never asked to produce
  `"texte"` in the live prompt, see the comment at the current call site —
  while leaving any OTHER recognized action, and the real `agressif` the
  model did send, untouched):

```rust
pub fn interpreter_ou_repli_libre(json_brut: &str, saisie_joueur: &str) -> domain::ActionJoueur {
    let action = interpreter_json_action(json_brut)
        .unwrap_or(domain::ActionJoueur::ActionLibre { texte: String::new(), agressif: false });
    match action {
        domain::ActionJoueur::ActionLibre { agressif, .. } => {
            domain::ActionJoueur::ActionLibre { texte: saisie_joueur.to_string(), agressif }
        }
        autre => autre,
    }
}
```

  Replace the two lines in `LlamaNarrateur::proposer_action`
  (`llama_narrateur.rs`, currently
  `interpreter_json_action(&json_brut).unwrap_or(ActionJoueur::ActionLibre(String::new()))`
  followed by the match that re-substitutes verbatim text) with a single
  call: `Ok(interpreter_ou_repli_libre(&json_brut, &contexte.saisie_joueur))`.
  Re-check the three Step 1 tests against this exact body: a parse error
  or an out-of-schema action both go through the `.unwrap_or(...)` default
  (`agressif: false`) and then the match's `ActionLibre` arm, ending up
  with `{ texte: saisie_joueur, agressif: false }` either way; a
  recognized `"libre"` with the model's own `agressif: true` would keep
  `true` through this same arm (not exercised by the Step 1 tests above,
  but worth being aware of when reviewing the diff); `"fuir"` and the other
  non-`ActionLibre` variants fall to `autre => autre`, unchanged.

- [ ] **Step 8: Add the classification prompt instruction**
  (`construire_prompt_classification` in `prompts.rs`): when `combat_actif`
  (reuse the existing `description_combat`-driven branching), change the
  `"libre"` example to `{"action": "libre", "agressif": true|false}` and add
  one instruction sentence: `agressif: true` = a physically hostile action
  (hitting, throwing something, charging); `agressif: false` = a non-violent
  one (talking, negotiating, observing, hiding...). Outside combat, the
  example stays `{"action": "libre"}`.

- [ ] **Step 9: Run and verify pass** — `cargo test --workspace` — expected:
  all pass, including the new tests from Step 1.

- [ ] **Step 10: Commit.**

```bash
git add crates/domain/src/action_joueur.rs crates/application/src/usecases/jouer_tour.rs \
        crates/infra/src/action_schema.rs crates/infra/src/prompts.rs \
        crates/infra/src/llama_narrateur.rs crates/infra/src/faux_narrateur.rs \
        crates/app_desktop/src/worker.rs
git commit -m "feat(combat): judge intent on free actions instead of always attacking"
```

---

## Task 3: Dynamic combat start via `ProvoquerCombat`

**Files:**
- Modify: `crates/domain/src/action_joueur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`
- Modify: `crates/infra/src/action_schema.rs` (`interpreter_json_action` only)
- Modify: `crates/infra/src/prompts.rs`

**Interfaces:**
- Consumes: `domain::{Difficulte, gabarit_ennemi}` (Task 1);
  `commencer_combat(personnage: &Personnage, ennemis: Vec<Ennemi>, de: &mut
  dyn GenerateurDe) -> Combat` (existing, unchanged).
- Produces: `domain::ActionJoueur::ProvoquerCombat { ennemi: String,
  difficulte: Difficulte }`; `resoudre_action_joueur`'s new signature
  `(etat: &mut EtatPartie, action: &ActionJoueur, de_joueur: &mut dyn
  GenerateurDe, de_monde: &mut dyn GenerateurDe) -> Vec<String>` (gains the
  `de_monde` parameter — both of `JouerTour::executer`'s call sites must be
  updated to pass it).

The old `amorcer_rencontre_initiale`/forced-opening path is **not** removed
in this task — it still runs. This task only adds the new mechanism so the
game is never left without any way to enter combat. Task 4 removes the old
path once this one is proven.

- [ ] **Step 1: Write the failing tests**, in
  `crates/application/src/usecases/jouer_tour.rs`'s test module:

```rust
#[test]
fn provoquer_combat_sans_combat_en_cours_en_demarre_un_avec_le_bon_gabarit() {
    let mut etat = etat_sans_combat();
    let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
    let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 5]); // init joueur, init ennemi
    let resume = resoudre_action_joueur(
        &mut etat,
        &ActionJoueur::ProvoquerCombat { ennemi: "Loup".into(), difficulte: Difficulte::Faible },
        &mut de_joueur,
        &mut de_monde,
    );
    let combat = etat.combat_en_cours.as_ref().expect("un combat doit démarrer");
    assert_eq!(combat.ennemis[0].nom, "Loup");
    assert_eq!(combat.ennemis[0].pv_max, 4); // palier Faible
    assert!(resume.iter().any(|l| l.contains("Loup")));
}

#[test]
fn provoquer_combat_alors_qu_un_combat_est_deja_en_cours_attaque_au_lieu_de_planter() {
    let mut etat = etat_en_combat(); // contient déjà un Gobelin
    let mut de_joueur = GenerateurDeSeed::nouveau(vec![15, 4]); // attaque, dégâts
    let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
    let resume = resoudre_action_joueur(
        &mut etat,
        &ActionJoueur::ProvoquerCombat { ennemi: "Gobelin".into(), difficulte: Difficulte::Fort },
        &mut de_joueur,
        &mut de_monde,
    );
    assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis.len(), 1); // pas de doublon
    assert!(!resume.is_empty());
}
```

In `crates/infra/src/action_schema.rs`'s test module:

```rust
#[test]
fn interprete_une_provocation_de_combat() {
    let action = interpreter_json_action(
        r#"{"action":"provoquer_combat","ennemi":"Loup","difficulte":"faible"}"#,
    ).unwrap();
    assert_eq!(action, ActionJoueur::ProvoquerCombat { ennemi: "Loup".into(), difficulte: Difficulte::Faible });
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test --workspace` —
  expected: compile errors (variant and new parameter don't exist yet).

- [ ] **Step 3: Add the variant** in `crates/domain/src/action_joueur.rs`:
  `ProvoquerCombat { ennemi: String, difficulte: Difficulte }` (import
  `Difficulte` from `crate::ennemi`).

- [ ] **Step 4: Add the `valider_action` arm** (`jouer_tour.rs`):
  `ActionJoueur::ProvoquerCombat { .. } => Ok(())` — always valid, this is
  what starts combat.

- [ ] **Step 5: Thread `de_monde` through `resoudre_action_joueur`** — add
  the parameter to its signature, and update both call sites in
  `JouerTour::executer` (the `fuite` branch and the `Acteur::Joueur` arm
  inside the turn-order loop) to pass the `de_monde` already in scope
  there.

- [ ] **Step 6: Implement the `ProvoquerCombat` arm** in
  `resoudre_action_joueur`: if `etat.combat_en_cours.is_none()`, build the
  `Ennemi` via `gabarit_ennemi(ennemi.clone(), *difficulte)`, call
  `commencer_combat(&etat.personnage, vec![cet_ennemi], de_monde)`, assign
  it to `etat.combat_en_cours`, push `format!("⚔️ {ennemi} surgit et
  t'attaque !")`. If a combat is already active, fall back to the same
  logic as the `Attaquer { cible }` arm (reuse it directly — pass `ennemi`
  as the target) rather than erroring.

- [ ] **Step 7: Add the `"provoquer_combat"` arm to
  `interpreter_json_action`** (`action_schema.rs`): read `"ennemi"` (string,
  required) and `"difficulte"` (string, one of `"faible"|"moyen"|"fort"`,
  required — unrecognized value is an `ErreurInterpretation`, same style as
  the existing missing-field errors).

- [ ] **Step 8: Add the format to `construire_prompt_classification`**
  (`prompts.rs`), only inside the "no combat active" branch (mirrors how
  `"esquiver"`/`"fuir"` are only added when combat *is* active):
  `{"action": "provoquer_combat", "ennemi": "NomDeLEnnemi", "difficulte": "faible"|"moyen"|"fort"}`,
  with the instruction from Global Constraints: use this only when the
  player's action is aggressive toward a named target or the situation
  calls for a sudden hostile encounter, and default to "faible" unless the
  player was clearly aggressive or the story already escalated.

- [ ] **Step 9: Run and verify pass** — `cargo test --workspace` —
  expected: all pass.

- [ ] **Step 10: Commit.**

```bash
git add crates/domain/src/action_joueur.rs crates/application/src/usecases/jouer_tour.rs \
        crates/infra/src/action_schema.rs crates/infra/src/prompts.rs
git commit -m "feat(combat): let the player's own aggression or the narrator start a fight"
```

---

## Task 4: Remove the forced opening encounter

**Files:**
- Modify: `crates/domain/src/etat_partie.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs`
- Modify: `crates/application/src/ports/sauvegarde.rs` (test fixture only)
- Modify: `crates/application/src/usecases/partie.rs` (test fixture only)
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app_desktop/src/worker.rs`
- Modify: `crates/infra/src/action_schema.rs` (test fixtures only)
- Modify: `crates/infra/src/faux_narrateur.rs` (test fixtures only)
- Modify: `crates/infra/src/llama_narrateur.rs` (test fixtures only)
- Modify: `crates/infra/src/prompts.rs` (test fixtures only)
- Modify: `crates/infra/src/sauvegarde_fichier.rs` (test fixtures only)

**Interfaces:**
- Consumes: Task 3's `ProvoquerCombat` (this is what makes removing the old
  forced path safe — combat can still start, just not unconditionally).
- Produces: `EtatPartie` with no `rencontre_initiale_declenchee` field — any
  later task constructing `EtatPartie` omits it.

This is the same kind of struct-literal sweep done in a previous session
for `ennemi_agit_avant_joueur` — mechanical, but every one of the 11 files
above has at least one `EtatPartie { ... }` literal that must drop the
field (confirmed by search; search again at implementation time in case
new ones were added since this plan was written).

- [ ] **Step 1: Write the failing test**, in
  `crates/application/src/usecases/jouer_tour.rs`'s test module:

```rust
#[test]
fn un_premier_tour_non_agressif_ne_demarre_aucun_combat() {
    let mut etat = etat_sans_combat();
    let mut narrateur = NarrateurDeTest {
        action: ActionJoueur::ActionLibre { texte: "j'observe les environs".into(), agressif: false },
    };
    let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
    let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
    JouerTour::executer(&mut etat, "j'observe les environs", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {})
        .unwrap();
    assert!(etat.combat_en_cours.is_none());
}
```

(Adjust to whatever the existing `NarrateurDeTest`/fixture names are in
this file — this test's point is that nothing forces combat anymore; it
replaces the old `jouer_tour_declenche_la_rencontre_initiale_au_tout_premier_tour`
test, which should be deleted since the behavior it pinned is gone.)

- [ ] **Step 2: Run and verify failure** — `cargo test -p application` —
  expected: the *existing* `jouer_tour_declenche_la_rencontre_initiale_au_tout_premier_tour`
  test still passes (nothing's changed yet) while the new test fails
  (combat still starts unconditionally).

- [ ] **Step 3: Delete `amorcer_rencontre_initiale`** and its call at the
  top of `JouerTour::executer`, and delete the old test
  `jouer_tour_declenche_la_rencontre_initiale_au_tout_premier_tour` plus any
  other test whose only purpose was pinning the forced-combat behavior
  (check `jouer_tour_ne_redeclenche_pas_la_rencontre_si_deja_amorcee` too —
  delete it, that guard no longer applies).

- [ ] **Step 4: Remove `rencontre_initiale_declenchee`** from
  `EtatPartie` (`etat_partie.rs`) and from every `EtatPartie { ... }`
  literal across the files listed above (mechanical sweep — grep for
  `rencontre_initiale_declenchee` to find every remaining site after Step
  3, update bottom-to-top per file if using a script, to avoid line-number
  drift).

- [ ] **Step 5: Run and verify pass** — `cargo test --workspace` —
  expected: all pass, including the new Step 1 test; zero remaining
  references to `rencontre_initiale_declenchee`
  (`grep -rn rencontre_initiale_declenchee crates/` returns nothing).

- [ ] **Step 6: Commit.**

```bash
git add -A
git commit -m "feat(combat): stop forcing a fight on the very first turn"
```

---

## Task 5: Teach the `*action*` vs speech convention

**Files:**
- Modify: `crates/infra/src/prompts.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing new (prompt text only — no signature changes).

- [ ] **Step 1: Write the failing tests**, in `crates/infra/src/prompts.rs`'s
  test module (mirror the existing style, e.g.
  `le_prompt_de_classification_contient_un_exemple_pour_chaque_format_d_action`):

```rust
#[test]
fn le_prompt_de_classification_explique_la_convention_asterisque() {
    let contexte = /* existing helper building a ContexteTour */;
    let prompt = construire_prompt_classification(&contexte);
    assert!(prompt.contains("astérisque"));
}

#[test]
fn le_prompt_de_narration_explique_la_convention_asterisque() {
    let contexte = /* existing helper building a ContexteResultat */;
    let prompt = construire_prompt_narration(&contexte);
    assert!(prompt.contains("astérisque"));
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test -p infra prompts::`
  — expected: both FAIL (no such text yet).

- [ ] **Step 3: Add one instruction sentence to each of
  `construire_prompt_classification` and `construire_prompt_narration`**:
  text wrapped in asterisks (`*...*`) describes a physical action/gesture
  by the player; text without asterisks is what the player says aloud;
  both must be taken into account, respecting which is which when
  classifying or narrating (e.g. don't render a physical action as quoted
  dialogue). Exact French wording is free.

- [ ] **Step 4: Run and verify pass** — `cargo test -p infra prompts::` —
  expected: both PASS; run `cargo test --workspace` once to confirm no
  other prompt-content test broke (several existing tests assert
  `prompt.contains(...)` on fixed substrings and could be sensitive to
  where the new sentence is inserted — if one breaks, move the new
  sentence rather than edit the older assertion).

- [ ] **Step 5: Commit.**

```bash
git add crates/infra/src/prompts.rs
git commit -m "feat(prompts): teach the model the *action* vs speech convention"
```

---

## Task 6: `Genre` + `PremisseHistoire` domain types and prompt injection

**Files:**
- Create: `crates/domain/src/genre.rs`
- Create: `crates/domain/src/premisse.rs`
- Modify: `crates/domain/src/etat_partie.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/application/src/ports/narrateur.rs`
- Modify: `crates/infra/src/prompts.rs`
- Modify: `crates/infra/src/llama_narrateur.rs`
- Modify: `crates/infra/src/faux_narrateur.rs`
- Modify: `crates/application/src/usecases/jouer_tour.rs` (test-double
  `raconter_introduction` impls only, ~4 occurrences)
- Modify: `crates/app_desktop/src/worker.rs` (call site + 3 test-double
  impls)
- Modify: `crates/app/src/main.rs` (call site)

**Interfaces:**
- Produces: `domain::Genre` enum with `nom_affiche(&self) -> &'static str`
  and `indication_prompt(&self) -> &'static str`, plus `pub const TOUTES:
  [Genre; 6]` (same shape as `app_desktop::modeles::PaletteModele`, see
  spec); `domain::PremisseHistoire { genre: Genre, description: String }`
  with a `Default` impl (`Genre::FantasyMedievale`, empty description);
  `EtatPartie.premisse: PremisseHistoire` (`#[serde(default)]`);
  `construire_prompt_introduction(personnage: &Personnage, lieu: &str,
  premisse: &PremisseHistoire) -> String` (gained the third parameter);
  `Narrateur::raconter_introduction`'s signature gains a `premisse: &
  PremisseHistoire` parameter (placed after `lieu`, before
  `sur_fragment`) — every implementor must be updated.
- Consumes: nothing from Tasks 1-5.

### Every `raconter_introduction` implementor/call site that must change
(verified by search — complete set)

Implementors (add the new parameter to the signature; the 7 test-double
ones that currently ignore their parameters just add
`_premisse: &domain::PremisseHistoire,` and keep ignoring it; the 2 real
ones use it):
- `crates/infra/src/llama_narrateur.rs:233` (real — passes it to
  `construire_prompt_introduction`).
- `crates/infra/src/faux_narrateur.rs:55` (real — can ignore it, or fold a
  short mention into its canned narration; developer's call).
- `crates/application/src/usecases/jouer_tour.rs:765, 1008, 1050, 1090`
  (test doubles — ignore it).
- `crates/app_desktop/src/worker.rs:532, 630, 705` (test doubles — ignore
  it).

Call sites:
- `crates/app_desktop/src/worker.rs:410` — pass `&etat_initial.premisse`.
- `crates/app/src/main.rs:36` — pass `&domain::PremisseHistoire::default()`
  (the console app has no UI for choosing a premise — explicit scope
  decision, see spec's non-goals; it always plays with the default
  genre/empty premise).

- [ ] **Step 1: Write the failing tests.**

In `crates/domain/src/premisse.rs`:

```rust
#[test]
fn premisse_par_defaut_est_fantasy_sans_description() {
    let p = PremisseHistoire::default();
    assert_eq!(p.genre, Genre::FantasyMedievale);
    assert_eq!(p.description, "");
}
```

In `crates/domain/src/etat_partie.rs`'s test module — extend
`etat_partie_fait_un_aller_retour_json` to also assert
`relu.premisse == etat.premisse`, and add:

```rust
#[test]
fn une_sauvegarde_sans_champ_premisse_se_relit_avec_la_valeur_par_defaut() {
    let etat = etat_test();
    let mut valeur = serde_json::to_value(&etat).unwrap();
    valeur.as_object_mut().unwrap().remove("premisse"); // simule une sauvegarde plus ancienne
    let relu: EtatPartie = serde_json::from_value(valeur).unwrap();
    assert_eq!(relu.premisse, PremisseHistoire::default());
}
```

In `crates/infra/src/prompts.rs`:

```rust
#[test]
fn le_prompt_d_introduction_mentionne_le_genre_et_la_premisse() {
    let premisse = PremisseHistoire { genre: Genre::Horreur, description: "une vieille malédiction".into() };
    let prompt = construire_prompt_introduction(&personnage_test(), "Clairière", &premisse);
    assert!(prompt.contains("une vieille malédiction"));
}

#[test]
fn le_prompt_de_narration_rappelle_la_premisse_a_chaque_tour() {
    let mut contexte = /* existing ContexteResultat helper */;
    contexte.etat.premisse = PremisseHistoire { genre: Genre::Horreur, description: "une vieille malédiction".into() };
    let prompt = construire_prompt_narration(&contexte);
    assert!(prompt.contains("une vieille malédiction"));
}

#[test]
fn la_premisse_est_tronquee_a_300_caracteres_dans_le_prompt() {
    let longue = "x".repeat(400);
    let premisse = PremisseHistoire { genre: Genre::FantasyMedievale, description: longue };
    let prompt = construire_prompt_introduction(&personnage_test(), "Clairière", &premisse);
    assert!(!prompt.contains(&"x".repeat(301)));
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test --workspace` —
  expected: compile errors (types don't exist, signatures don't match).

- [ ] **Step 3: Create `crates/domain/src/genre.rs`** — mirror
  `app_desktop::modeles::PaletteModele`'s shape exactly:
  `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]`,
  `#[serde(rename_all = "snake_case")]`, variants
  `FantasyMedievale, ScienceFiction, Horreur, PolarNoir,
  PostApocalyptique, Steampunk`, `pub const TOUTES: [Genre; 6] = [...]`,
  `nom_affiche()` (French display labels, developer's wording) and
  `indication_prompt()` (one short clause per genre for the prompt, e.g.
  Science-Fiction → "un futur de vaisseaux spatiaux et de technologies
  avancées"). Implement `Default` as `Genre::FantasyMedievale`.

- [ ] **Step 4: Create `crates/domain/src/premisse.rs`** —
  `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
  pub struct PremisseHistoire { pub genre: Genre, pub description: String }`
  (derived `Default` works once `Genre` has its own `Default`).

- [ ] **Step 5: Add `premisse: PremisseHistoire` to `EtatPartie`** with
  `#[serde(default)]`, and update its one existing test fixture
  (`etat_test()` in `etat_partie.rs`) plus every other `EtatPartie { ... }`
  literal across the codebase (same sweep style as Task 4 — search for
  `EtatPartie {` to find every site still missing the field after this
  step).

- [ ] **Step 6: Export from `crates/domain/src/lib.rs`** — add `mod genre;
  mod premisse;` and `pub use genre::Genre; pub use premisse::PremisseHistoire;`.

- [ ] **Step 7: Change `construire_prompt_introduction`'s signature**
  (`prompts.rs`) to take `premisse: &PremisseHistoire`, and inject
  `premisse.genre.indication_prompt()` plus (if non-empty)
  `premisse.description` into the generated text — truncate the
  description to its first 300 chars before interpolating (new private
  helper, e.g. `fn tronquer(s: &str, max: usize) -> &str`, used here and
  in the Step 8 helper).

- [ ] **Step 8: Add a per-turn reminder to `construire_prompt_narration`**
  — a new private helper `description_premisse(etat: &EtatPartie) ->
  String` (same style as `description_memoire`), returning an empty string
  when the premise is the default (no genre customization signal needed if
  it's exactly `PremisseHistoire::default()`... developer's call: simplest
  is to always include at least the genre's `indication_prompt()`, and the
  free description only when non-empty, truncated as in Step 7), wired into
  `construire_prompt_narration`'s format string.

- [ ] **Step 9: Change `Narrateur::raconter_introduction`'s signature**
  (`crates/application/src/ports/narrateur.rs`) to add `premisse: &
  domain::PremisseHistoire` as its third parameter (after `lieu`, before
  `sur_fragment`).

- [ ] **Step 10: Fix every implementor and call site** listed above.

- [ ] **Step 11: Run and verify pass** — `cargo test --workspace` —
  expected: all pass.

- [ ] **Step 12: Commit.**

```bash
git add -A
git commit -m "feat(premisse): add genre and free-text premise, threaded into every prompt"
```

---

## Task 7: UI for genre and premise

**Files:**
- Modify: `crates/app_desktop/src/vues/creation.rs`
- Modify: `crates/app_desktop/src/protocole.rs`
- Modify: `crates/app_desktop/src/worker.rs`

**Interfaces:**
- Consumes: `domain::{Genre, PremisseHistoire}` (Task 6).
- Produces: `MessageVersWorker::CreerPersonnage` gains `genre: Genre,
  premisse_libre: String`; `worker::creer_nouvelle_histoire`'s signature
  gains `genre: Genre, premisse_libre: String` and builds
  `EtatPartie.premisse = PremisseHistoire { genre, description:
  premisse_libre }` verbatim, untruncated. The two truncation layers the
  spec actually calls for are the UI `maxlength` (Step 6 below) and
  Task 6's prompt-build-time truncation (`description_premisse`/
  `construire_prompt_introduction`, which runs on `EtatPartie.premisse`
  regardless of how it got there — already covers this path and every
  other, so do not add a third truncation point here.

- [ ] **Step 1: Write the failing test**, in `crates/app_desktop/src/
  worker.rs`'s test module (mirrors the existing
  `creer_nouvelle_histoire_construit_un_personnage_sans_combat_force` test):

```rust
#[test]
fn creer_nouvelle_histoire_porte_le_genre_et_la_premisse_choisis() {
    let etat = creer_nouvelle_histoire(
        "Elowen", "Guerrier", [15, 14, 13, 12, 10, 8],
        Genre::Horreur, "une vieille malédiction".into(),
    ).unwrap();
    assert_eq!(etat.premisse.genre, Genre::Horreur);
    assert_eq!(etat.premisse.description, "une vieille malédiction");
}
```

- [ ] **Step 2: Run and verify failure** — `cargo test -p app_desktop` —
  expected: compile error (extra arguments don't match current signature).

- [ ] **Step 3: Update `creer_nouvelle_histoire`'s signature** (`worker.rs`)
  and its one existing caller (the `CreerPersonnage` message handler),
  building `EtatPartie.premisse` as described above. Update the existing
  `creer_nouvelle_histoire_construit_un_personnage_sans_combat_force` test
  call site to pass `Genre::default(), String::new()`.

- [ ] **Step 4: Add the two fields to `MessageVersWorker::CreerPersonnage`**
  (`protocole.rs`): `genre: Genre, premisse_libre: String`.

- [ ] **Step 5: Run and verify pass (backend)** — `cargo test -p app_desktop`
  — expected: all pass.

- [ ] **Step 6: Add the UI fields to `Creation`** (`creation.rs`): a
  `genre: Signal<Genre>` initialized to `Genre::default()` driving a
  `<select>` over `Genre::TOUTES` (exact same pattern as the existing
  `Classe`/`Modèle` selects — static list, no dynamic-option churn risk);
  a `premisse_libre: Signal<String>` driving a `<textarea maxlength="300">`
  (optional, empty default). Pass both through the `CreerPersonnage`
  message built on "Commencer l'aventure".

- [ ] **Step 7: Compile-check the UI** — `cargo check -p app_desktop`.

- [ ] **Step 8: Run the full workspace suite** — `cargo test --workspace`
  — expected: all pass, 0 warnings
  (`cargo build --workspace --all-targets 2>&1 | grep -i warning` empty).

- [ ] **Step 9: Commit.**

```bash
git add crates/app_desktop/src/vues/creation.rs crates/app_desktop/src/protocole.rs \
        crates/app_desktop/src/worker.rs
git commit -m "feat(app_desktop): let the player pick a genre and write a free premise"
```

---

## Final check

- [ ] `cargo test --workspace` green.
- [ ] `cargo clippy --workspace --all-targets` — no new warnings beyond the
  two pre-existing, unrelated ones already known (`main.rs` thread_local).
- [ ] `grep -rn rencontre_initiale_declenchee crates/` — empty.
- [ ] `grep -rn "ActionLibre(" crates/` — empty (every remaining reference
  uses the struct-variant form).
- [ ] Manual read-through of `construire_prompt_classification`'s full
  output for a combat-active context, to confirm the `provoquer_combat`,
  `agressif`, and asterisk-convention additions don't visually run into
  each other (this prompt has grown across three tasks).
