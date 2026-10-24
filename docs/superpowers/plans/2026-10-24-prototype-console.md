# Prototype Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a playable, terminal-driven JDR solo loop (création de personnage, combat au tour par tour contre un gobelin, sauvegarde/reprise) across a 4-crate clean-architecture workspace, with a `FauxNarrateur` standing in for the future `llama-cpp-2` integration.

**Architecture:** `domain` holds pure entities and rules (no external deps but `serde`). `application` defines ports (`Narrateur`, `GenerateurDe`, `Sauvegarde`) and use cases that orchestrate `domain` through those ports. `infra` implements the ports (`FauxNarrateur`, `GenerateurDeOs`, `SauvegardeFichier`). `app` is a CLI binary that wires everything together and does terminal I/O.

**Tech Stack:** Rust (edition 2021), `serde`/`serde_json` for state + save files, `rand` 0.8 for the real dice RNG. No async, no Dioxus, no LLM yet.

**Spec:** `docs/superpowers/specs/2026-10-24-prototype-console-design.md`

## Global Constraints

- Workspace has exactly 4 member crates: `domain`, `application`, `infra`, `app` (path `crates/<name>`), per the spec's clean-architecture rule: a crate only depends on crates closer to the domain, never the reverse.
- `domain` depends on nothing external except `serde` (with `derive`). No `rand`, no `serde_json`, no I/O in `domain`.
- Dice are never rolled directly by `domain` — `domain` only receives already-rolled totals/`JetDe` values and exposes pure resolution functions.
- The LLM/`llama-cpp-2` integration is out of scope for this plan — `Narrateur` is only implemented by `infra::FauxNarrateur`.
- Only the player's action goes through narrative interpretation; enemies always act via deterministic code (no LLM call for enemy turns).
- Release profile in the workspace `Cargo.toml`: `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (copied from the brief).
- Standard array for caractéristiques: `[15, 14, 13, 12, 10, 8]`, assigned by the player to `force, dexterite, constitution, intelligence, sagesse, charisme]` in that order. Valid classes: `["Guerrier", "Mage", "Voleur"]`.
- `pv_max = (10 + modificateur(constitution)).max(1)`. `defense = 10 + modificateur(dexterite)`. `modificateur(valeur) = (valeur as i32 - 10).div_euclid(2)`.
- Fixed prototype encounter: one `Ennemi` "Gobelin" with `pv_max=7`, `degats=1d6`, `defense=12`, `bonus_initiative=2`, `bonus_attaque=3`.

---

## Task 1: Workspace scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/domain/Cargo.toml`, `crates/domain/src/lib.rs`
- Create: `crates/application/Cargo.toml`, `crates/application/src/lib.rs`
- Create: `crates/infra/Cargo.toml`, `crates/infra/src/lib.rs`
- Create: `crates/app/Cargo.toml`, `crates/app/src/main.rs`
- Create: `.gitignore`

**Interfaces:**
- Produces: four buildable, empty crates named `domain`, `application`, `infra`, `app`, wired with the path dependencies later tasks will use (`application` depends on `domain`; `infra` depends on `domain` + `application`; `app` depends on all three).

- [ ] **Step 1: Create the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/domain",
    "crates/application",
    "crates/infra",
    "crates/app",
]

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

- [ ] **Step 2: Create `.gitignore`**

```
/target
partie.json
```

- [ ] **Step 3: Create `crates/domain/Cargo.toml` and empty `lib.rs`**

```toml
[package]
name = "domain"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
```

`crates/domain/src/lib.rs` is empty for now (will be filled in Task 2+).

- [ ] **Step 4: Create `crates/application/Cargo.toml` and empty `lib.rs`**

```toml
[package]
name = "application"
version = "0.1.0"
edition = "2021"

[dependencies]
domain = { path = "../domain" }
serde = { version = "1", features = ["derive"] }
```

- [ ] **Step 5: Create `crates/infra/Cargo.toml` and empty `lib.rs`**

```toml
[package]
name = "infra"
version = "0.1.0"
edition = "2021"

[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
serde_json = "1"
rand = "0.8"
```

- [ ] **Step 6: Create `crates/app/Cargo.toml` and a placeholder `main.rs`**

```toml
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
infra = { path = "../infra" }
```

`crates/app/src/main.rs`:

```rust
fn main() {
    println!("Narrateur — prototype console");
}
```

- [ ] **Step 7: Verify the workspace builds**

Run: `cargo build`
Expected: Compiles successfully, 4 crates built, no errors.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml .gitignore crates
git commit -m "chore: scaffold 4-crate workspace for the console prototype"
```

---

## Task 2: Domain — dés (`De`, `JetDe`)

**Files:**
- Create: `crates/domain/src/de.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Produces: `domain::De { faces: u32 }` (Copy), `domain::JetDe { resultat: u32, modificateur: i32, total: i32 }` (Copy) with constructor `JetDe::nouveau(resultat: u32, modificateur: i32) -> JetDe`.

- [ ] **Step 1: Write the failing test**

In `crates/domain/src/de.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jet_de_calcule_le_total_avec_modificateur() {
        let jet = JetDe::nouveau(14, 2);
        assert_eq!(jet.resultat, 14);
        assert_eq!(jet.modificateur, 2);
        assert_eq!(jet.total, 16);
    }

    #[test]
    fn jet_de_total_peut_etre_negatif() {
        let jet = JetDe::nouveau(1, -3);
        assert_eq!(jet.total, -2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain de::tests`
Expected: FAIL to compile — `De`/`JetDe` not defined.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/de.rs` (full file, tests included above go at the bottom):

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct De {
    pub faces: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JetDe {
    pub resultat: u32,
    pub modificateur: i32,
    pub total: i32,
}

impl JetDe {
    pub fn nouveau(resultat: u32, modificateur: i32) -> Self {
        Self {
            resultat,
            modificateur,
            total: resultat as i32 + modificateur,
        }
    }
}
```

`crates/domain/src/lib.rs`:

```rust
mod de;

pub use de::{De, JetDe};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/de.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add De and JetDe value objects"
```

---

## Task 3: Domain — `Caracteristiques` et `modificateur`

**Files:**
- Create: `crates/domain/src/caracteristiques.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `domain::Caracteristiques { force, dexterite, constitution, intelligence, sagesse, charisme: u32 }` (Copy), `domain::modificateur(valeur: u32) -> i32`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modificateur_de_quinze_est_plus_deux() {
        assert_eq!(modificateur(15), 2);
    }

    #[test]
    fn modificateur_de_dix_est_zero() {
        assert_eq!(modificateur(10), 0);
    }

    #[test]
    fn modificateur_de_huit_est_moins_un() {
        assert_eq!(modificateur(8), -1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain caracteristiques::tests`
Expected: FAIL to compile — module doesn't exist yet.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/caracteristiques.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caracteristiques {
    pub force: u32,
    pub dexterite: u32,
    pub constitution: u32,
    pub intelligence: u32,
    pub sagesse: u32,
    pub charisme: u32,
}

pub fn modificateur(valeur: u32) -> i32 {
    (valeur as i32 - 10).div_euclid(2)
}
```

Update `crates/domain/src/lib.rs`:

```rust
mod caracteristiques;
mod de;

pub use caracteristiques::{modificateur, Caracteristiques};
pub use de::{De, JetDe};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (5 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/caracteristiques.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add Caracteristiques and the modificateur formula"
```

---

## Task 4: Domain — `Objet` et `EffetObjet`

**Files:**
- Create: `crates/domain/src/objet.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Consumes: `domain::De` (Task 2).
- Produces: `domain::EffetObjet::{Arme { degats: De }, Consommable { soin: u32 }}`, `domain::Objet { nom, description, effet: Option<EffetObjet> }`, `Objet::epee() -> Objet`, `Objet::potion_de_soin() -> Objet`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::De;

    #[test]
    fn epee_standard_infflige_un_d8() {
        let epee = Objet::epee();
        assert_eq!(epee.effet, Some(EffetObjet::Arme { degats: De { faces: 8 } }));
    }

    #[test]
    fn potion_de_soin_soigne_dix_points() {
        let potion = Objet::potion_de_soin();
        assert_eq!(potion.effet, Some(EffetObjet::Consommable { soin: 10 }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain objet::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/objet.rs`:

```rust
use crate::de::De;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EffetObjet {
    Arme { degats: De },
    Consommable { soin: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objet {
    pub nom: String,
    pub description: String,
    pub effet: Option<EffetObjet>,
}

impl Objet {
    pub fn epee() -> Self {
        Objet {
            nom: "Épée".to_string(),
            description: "Une épée simple mais efficace.".to_string(),
            effet: Some(EffetObjet::Arme { degats: De { faces: 8 } }),
        }
    }

    pub fn potion_de_soin() -> Self {
        Objet {
            nom: "Potion de soin".to_string(),
            description: "Un liquide rougeâtre qui referme les blessures.".to_string(),
            effet: Some(EffetObjet::Consommable { soin: 10 }),
        }
    }
}
```

Update `crates/domain/src/lib.rs` to add `mod objet;` and `pub use objet::{EffetObjet, Objet};`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (7 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/objet.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add Objet and EffetObjet with starting-gear constructors"
```

---

## Task 5: Domain — `Personnage`

**Files:**
- Create: `crates/domain/src/personnage.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Consumes: `Caracteristiques` (Task 3), `modificateur` (Task 3), `Objet`/`EffetObjet`/`De` (Tasks 2, 4).
- Produces: `domain::Personnage { nom, classe, caracteristiques, pv_actuels, pv_max, inventaire: Vec<Objet> }` with `Personnage::nouveau(nom, classe, caracteristiques, inventaire) -> Personnage`, `.defense() -> i32`, `.subir_degats(i32)`, `.soigner(i32)`, `.est_mort() -> bool`, `.arme_equipee() -> De`, `.retirer_objet(&str) -> Option<Objet>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::objet::Objet;

    fn caracteristiques_test() -> Caracteristiques {
        Caracteristiques {
            force: 12,
            dexterite: 14,
            constitution: 13,
            intelligence: 10,
            sagesse: 10,
            charisme: 8,
        }
    }

    #[test]
    fn nouveau_calcule_pv_max_depuis_la_constitution() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.pv_max, 11); // 10 + modificateur(13) = 10 + 1
        assert_eq!(perso.pv_actuels, 11);
    }

    #[test]
    fn defense_depend_de_la_dexterite() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.defense(), 12); // 10 + modificateur(14) = 10 + 2
    }

    #[test]
    fn subir_degats_est_borne_a_zero() {
        let mut perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        perso.subir_degats(100);
        assert_eq!(perso.pv_actuels, 0);
        assert!(perso.est_mort());
    }

    #[test]
    fn soigner_est_borne_au_pv_max() {
        let mut perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        perso.subir_degats(5);
        perso.soigner(100);
        assert_eq!(perso.pv_actuels, perso.pv_max);
    }

    #[test]
    fn arme_equipee_retourne_mains_nues_sans_arme() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.arme_equipee(), De { faces: 4 });
    }

    #[test]
    fn arme_equipee_retourne_la_premiere_arme_trouvee() {
        let perso = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            caracteristiques_test(),
            vec![Objet::epee(), Objet::potion_de_soin()],
        );
        assert_eq!(perso.arme_equipee(), De { faces: 8 });
    }

    #[test]
    fn retirer_objet_enleve_et_retourne_l_objet() {
        let mut perso = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            caracteristiques_test(),
            vec![Objet::potion_de_soin()],
        );
        let retire = perso.retirer_objet("Potion de soin");
        assert!(retire.is_some());
        assert!(perso.inventaire.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain personnage::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/personnage.rs`:

```rust
use crate::caracteristiques::{modificateur, Caracteristiques};
use crate::de::De;
use crate::objet::{EffetObjet, Objet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personnage {
    pub nom: String,
    pub classe: String,
    pub caracteristiques: Caracteristiques,
    pub pv_actuels: i32,
    pub pv_max: i32,
    pub inventaire: Vec<Objet>,
}

impl Personnage {
    pub fn nouveau(
        nom: String,
        classe: String,
        caracteristiques: Caracteristiques,
        inventaire: Vec<Objet>,
    ) -> Self {
        let pv_max = (10 + modificateur(caracteristiques.constitution)).max(1);
        Self {
            nom,
            classe,
            caracteristiques,
            pv_actuels: pv_max,
            pv_max,
            inventaire,
        }
    }

    pub fn defense(&self) -> i32 {
        10 + modificateur(self.caracteristiques.dexterite)
    }

    pub fn subir_degats(&mut self, degats: i32) {
        self.pv_actuels = (self.pv_actuels - degats).max(0);
    }

    pub fn soigner(&mut self, soin: i32) {
        self.pv_actuels = (self.pv_actuels + soin).min(self.pv_max);
    }

    pub fn est_mort(&self) -> bool {
        self.pv_actuels <= 0
    }

    pub fn arme_equipee(&self) -> De {
        self.inventaire
            .iter()
            .find_map(|o| match &o.effet {
                Some(EffetObjet::Arme { degats }) => Some(*degats),
                _ => None,
            })
            .unwrap_or(De { faces: 4 })
    }

    pub fn retirer_objet(&mut self, nom: &str) -> Option<Objet> {
        let position = self.inventaire.iter().position(|o| o.nom == nom)?;
        Some(self.inventaire.remove(position))
    }
}
```

Update `crates/domain/src/lib.rs` to add `mod personnage;` and `pub use personnage::Personnage;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (14 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/personnage.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add Personnage with HP, defense, inventory rules"
```

---

## Task 6: Domain — `Ennemi`

**Files:**
- Create: `crates/domain/src/ennemi.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Consumes: `De` (Task 2).
- Produces: `domain::Ennemi { nom, pv_actuels, pv_max, degats: De, defense, bonus_initiative, bonus_attaque: i32 }` with `Ennemi::nouveau(nom, pv_max, degats, defense, bonus_initiative, bonus_attaque) -> Ennemi`, `.subir_degats(i32)`, `.est_mort() -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::De;

    #[test]
    fn nouveau_initialise_les_pv_actuels_au_max() {
        let gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        assert_eq!(gobelin.pv_actuels, 7);
        assert_eq!(gobelin.pv_max, 7);
    }

    #[test]
    fn subir_degats_est_borne_a_zero() {
        let mut gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        gobelin.subir_degats(100);
        assert_eq!(gobelin.pv_actuels, 0);
        assert!(gobelin.est_mort());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain ennemi::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/ennemi.rs`:

```rust
use crate::de::De;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ennemi {
    pub nom: String,
    pub pv_actuels: i32,
    pub pv_max: i32,
    pub degats: De,
    pub defense: i32,
    pub bonus_initiative: i32,
    pub bonus_attaque: i32,
}

impl Ennemi {
    pub fn nouveau(
        nom: String,
        pv_max: i32,
        degats: De,
        defense: i32,
        bonus_initiative: i32,
        bonus_attaque: i32,
    ) -> Self {
        Self {
            nom,
            pv_actuels: pv_max,
            pv_max,
            degats,
            defense,
            bonus_initiative,
            bonus_attaque,
        }
    }

    pub fn subir_degats(&mut self, degats: i32) {
        self.pv_actuels = (self.pv_actuels - degats).max(0);
    }

    pub fn est_mort(&self) -> bool {
        self.pv_actuels <= 0
    }
}
```

Update `crates/domain/src/lib.rs` to add `mod ennemi;` and `pub use ennemi::Ennemi;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (16 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/ennemi.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add Ennemi"
```

---

## Task 7: Domain — `ActionJoueur` et `Combat`

**Files:**
- Create: `crates/domain/src/action_joueur.rs`
- Create: `crates/domain/src/combat.rs`
- Modify: `crates/domain/src/lib.rs`

**Interfaces:**
- Consumes: `Ennemi` (Task 6), `JetDe` (Task 2).
- Produces: `domain::ActionJoueur::{Attaquer { cible: String }, UtiliserObjet { objet: String }, Fuir, ActionLibre(String)}`; `domain::Vainqueur::{Joueur, Ennemis, Fuite}` (Copy); `domain::Combat { ennemis: Vec<Ennemi>, ennemi_agit_avant_joueur: bool, tour_actuel: u32, termine: bool, vainqueur: Option<Vainqueur> }` with `.tous_ennemis_morts() -> bool`, `.trouver_ennemi_vivant_mut(&str) -> Option<&mut Ennemi>`, `.terminer(Vainqueur)`; `domain::ResolutionAttaque { touche: bool, degats: i32 }`; `domain::resoudre_attaque(attaque_totale: i32, defense_cible: i32, jet_degats: JetDe) -> ResolutionAttaque`; `domain::determiner_ordre_initiative(initiative_joueur: i32, initiatives_ennemis: &[i32]) -> bool`.

- [ ] **Step 1: Write the failing tests**

`crates/domain/src/action_joueur.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ActionJoueur {
    Attaquer { cible: String },
    UtiliserObjet { objet: String },
    Fuir,
    ActionLibre(String),
}
```

(No dedicated tests for this plain enum — it's exercised by `combat.rs`'s tests below and later tasks.)

`crates/domain/src/combat.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::{De, JetDe};
    use crate::ennemi::Ennemi;

    fn gobelin() -> Ennemi {
        Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)
    }

    #[test]
    fn resoudre_attaque_touche_si_total_superieur_ou_egal_a_la_defense() {
        let resolution = resoudre_attaque(12, 12, JetDe::nouveau(5, 0));
        assert!(resolution.touche);
        assert_eq!(resolution.degats, 5);
    }

    #[test]
    fn resoudre_attaque_rate_si_total_inferieur_a_la_defense() {
        let resolution = resoudre_attaque(11, 12, JetDe::nouveau(5, 0));
        assert!(!resolution.touche);
        assert_eq!(resolution.degats, 0);
    }

    #[test]
    fn determiner_ordre_initiative_vrai_si_un_ennemi_est_plus_rapide() {
        assert!(determiner_ordre_initiative(10, &[15]));
        assert!(!determiner_ordre_initiative(15, &[10]));
        assert!(!determiner_ordre_initiative(10, &[10]));
    }

    #[test]
    fn tous_ennemis_morts_vrai_quand_pv_a_zero() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        assert!(!combat.tous_ennemis_morts());
        combat.ennemis[0].subir_degats(100);
        assert!(combat.tous_ennemis_morts());
    }

    #[test]
    fn trouver_ennemi_vivant_mut_ignore_les_morts() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        combat.ennemis[0].subir_degats(100);
        assert!(combat.trouver_ennemi_vivant_mut("Gobelin").is_none());
    }

    #[test]
    fn terminer_fixe_le_vainqueur_et_le_flag_termine() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        combat.terminer(Vainqueur::Joueur);
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Joueur));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain combat::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

`crates/domain/src/combat.rs` (non-test code):

```rust
use crate::de::JetDe;
use crate::ennemi::Ennemi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vainqueur {
    Joueur,
    Ennemis,
    Fuite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Combat {
    pub ennemis: Vec<Ennemi>,
    pub ennemi_agit_avant_joueur: bool,
    pub tour_actuel: u32,
    pub termine: bool,
    pub vainqueur: Option<Vainqueur>,
}

impl Combat {
    pub fn tous_ennemis_morts(&self) -> bool {
        self.ennemis.iter().all(|e| e.est_mort())
    }

    pub fn trouver_ennemi_vivant_mut(&mut self, nom: &str) -> Option<&mut Ennemi> {
        self.ennemis.iter_mut().find(|e| e.nom == nom && !e.est_mort())
    }

    pub fn terminer(&mut self, vainqueur: Vainqueur) {
        self.termine = true;
        self.vainqueur = Some(vainqueur);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionAttaque {
    pub touche: bool,
    pub degats: i32,
}

pub fn resoudre_attaque(attaque_totale: i32, defense_cible: i32, jet_degats: JetDe) -> ResolutionAttaque {
    let touche = attaque_totale >= defense_cible;
    let degats = if touche { jet_degats.total.max(0) } else { 0 };
    ResolutionAttaque { touche, degats }
}

pub fn determiner_ordre_initiative(initiative_joueur: i32, initiatives_ennemis: &[i32]) -> bool {
    initiatives_ennemis.iter().any(|&i| i > initiative_joueur)
}
```

Update `crates/domain/src/lib.rs`:

```rust
mod action_joueur;
mod caracteristiques;
mod combat;
mod de;
mod ennemi;
mod objet;
mod personnage;

pub use action_joueur::ActionJoueur;
pub use caracteristiques::{modificateur, Caracteristiques};
pub use combat::{determiner_ordre_initiative, resoudre_attaque, Combat, ResolutionAttaque, Vainqueur};
pub use de::{De, JetDe};
pub use ennemi::Ennemi;
pub use objet::{EffetObjet, Objet};
pub use personnage::Personnage;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (22 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/action_joueur.rs crates/domain/src/combat.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add ActionJoueur and Combat with attack/initiative resolution"
```

---

## Task 8: Domain — `EtatPartie`

**Files:**
- Create: `crates/domain/src/etat_partie.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/domain/Cargo.toml` (dev-dependency for the round-trip test)

**Interfaces:**
- Consumes: `Personnage` (Task 5), `Combat` (Task 7).
- Produces: `domain::EtatPartie { personnage: Personnage, lieu_actuel: String, combat_en_cours: Option<Combat>, resumes: Vec<String> }`, fully `Serialize`/`Deserialize`.

- [ ] **Step 1: Write the failing test**

Add to `crates/domain/Cargo.toml`:

```toml
[dev-dependencies]
serde_json = "1"
```

`crates/domain/src/etat_partie.rs`:

```rust
use crate::combat::Combat;
use crate::personnage::Personnage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
}

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
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain etat_partie::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Implementation is already written above** — add the module wiring.

Update `crates/domain/src/lib.rs` to add `mod etat_partie;` and `pub use etat_partie::EtatPartie;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain`
Expected: PASS (23 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/etat_partie.rs crates/domain/src/lib.rs crates/domain/Cargo.toml
git commit -m "feat(domain): add EtatPartie as the full serializable game state"
```

---

## Task 9: Application — ports `Narrateur` et `GenerateurDe`

**Files:**
- Create: `crates/application/src/ports/mod.rs`
- Create: `crates/application/src/ports/narrateur.rs`
- Create: `crates/application/src/ports/generateur_de.rs`
- Modify: `crates/application/src/lib.rs`

**Interfaces:**
- Consumes: `domain::{ActionJoueur, De, EtatPartie, JetDe}`.
- Produces: `application::ports::narrateur::{Narrateur, ContexteTour, ReponseMj, ErreurNarrateur}`; `application::ports::generateur_de::GenerateurDe` with `fn lancer(&mut self, de: De, modificateur: i32) -> JetDe`; a `#[cfg(test)]` `GenerateurDeSeed` fake in the same file for use by later application tests.

- [ ] **Step 1: Write the failing test**

`crates/application/src/ports/generateur_de.rs`:

```rust
use domain::{De, JetDe};

pub trait GenerateurDe {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe;
}

#[cfg(test)]
pub struct GenerateurDeSeed {
    resultats: std::collections::VecDeque<u32>,
}

#[cfg(test)]
impl GenerateurDeSeed {
    pub fn nouveau(resultats: Vec<u32>) -> Self {
        Self { resultats: resultats.into() }
    }
}

#[cfg(test)]
impl GenerateurDe for GenerateurDeSeed {
    fn lancer(&mut self, _de: De, modificateur: i32) -> JetDe {
        let resultat = self
            .resultats
            .pop_front()
            .expect("plus de résultats programmés dans GenerateurDeSeed");
        JetDe::nouveau(resultat, modificateur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generateur_de_seed_renvoie_les_resultats_dans_l_ordre() {
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        assert_eq!(de.lancer(De { faces: 20 }, 2).total, 12);
        assert_eq!(de.lancer(De { faces: 6 }, 0).total, 3);
    }

    #[test]
    #[should_panic(expected = "plus de résultats programmés")]
    fn generateur_de_seed_panique_si_epuise() {
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        de.lancer(De { faces: 20 }, 0);
    }
}
```

`crates/application/src/ports/narrateur.rs`:

```rust
use domain::{ActionJoueur, EtatPartie};

#[derive(Debug, Clone)]
pub struct ContexteTour {
    pub etat: EtatPartie,
    pub saisie_joueur: String,
}

#[derive(Debug, Clone)]
pub struct ReponseMj {
    pub narration: String,
    pub action_proposee: ActionJoueur,
}

#[derive(Debug)]
pub enum ErreurNarrateur {
    EchecGeneration(String),
}

pub trait Narrateur: Send {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur>;
}
```

(No dedicated unit test for `narrateur.rs` — it's a pure trait/data definition, exercised by Task 13+'s tests and Task 17's `FauxNarrateur` tests.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application generateur_de::tests`
Expected: FAIL to compile — `application` crate and its modules don't exist yet.

- [ ] **Step 3: Wire the module tree**

`crates/application/src/ports/mod.rs`:

```rust
pub mod generateur_de;
pub mod narrateur;
```

`crates/application/src/lib.rs`:

```rust
pub mod ports;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src
git commit -m "feat(application): add Narrateur and GenerateurDe ports"
```

---

## Task 10: Application — port `Sauvegarde`

**Files:**
- Create: `crates/application/src/ports/sauvegarde.rs`
- Modify: `crates/application/src/ports/mod.rs`

**Interfaces:**
- Consumes: `domain::EtatPartie`.
- Produces: `application::ports::sauvegarde::{Sauvegarde, ErreurSauvegarde}` with `fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde>`, `fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde>`.

- [ ] **Step 1: Write the failing test**

This port is a trait with no logic of its own — it's exercised end-to-end in Task 16 (use cases) and Task 19 (`SauvegardeFichier`). Write a compile-level test using an in-memory fake directly in this file to lock the contract down now:

```rust
use domain::EtatPartie;

#[derive(Debug)]
pub enum ErreurSauvegarde {
    FichierAbsent,
    Lecture(String),
    Ecriture(String),
}

pub trait Sauvegarde {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde>;
    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Personnage};
    use std::cell::RefCell;

    struct SauvegardeMemoire {
        contenu: RefCell<Option<EtatPartie>>,
    }

    impl Sauvegarde for SauvegardeMemoire {
        fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
            self.contenu
                .borrow()
                .clone()
                .ok_or(ErreurSauvegarde::FichierAbsent)
        }

        fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
            *self.contenu.borrow_mut() = Some(etat.clone());
            Ok(())
        }
    }

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
            resumes: vec![],
        }
    }

    #[test]
    fn charger_sans_sauvegarde_prealable_echoue() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        assert!(matches!(sauvegarde.charger(), Err(ErreurSauvegarde::FichierAbsent)));
    }

    #[test]
    fn sauvegarder_puis_charger_renvoie_le_meme_etat() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        let etat = etat_test();
        sauvegarde.sauvegarder(&etat).unwrap();
        let relu = sauvegarde.charger().unwrap();
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application sauvegarde::tests`
Expected: FAIL to compile — module doesn't exist.

- [ ] **Step 3: Wire the module**

Update `crates/application/src/ports/mod.rs`:

```rust
pub mod generateur_de;
pub mod narrateur;
pub mod sauvegarde;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (4 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/ports/sauvegarde.rs crates/application/src/ports/mod.rs
git commit -m "feat(application): add Sauvegarde port"
```

---

## Task 11: Application — cas d'usage `creer_personnage`

**Files:**
- Create: `crates/application/src/usecases/mod.rs`
- Create: `crates/application/src/usecases/creer_personnage.rs`
- Modify: `crates/application/src/lib.rs`

**Interfaces:**
- Consumes: `domain::{Caracteristiques, Objet, Personnage}`.
- Produces: `application::usecases::creer_personnage::{creer_personnage, CLASSES_VALIDES, VALEURS_STANDARD, ErreurCreationPersonnage}` where `creer_personnage(nom: &str, classe: &str, valeurs: [u32; 6]) -> Result<Personnage, ErreurCreationPersonnage>`. `valeurs` order is `[force, dexterite, constitution, intelligence, sagesse, charisme]`.

- [ ] **Step 1: Write the failing tests**

```rust
use domain::{Caracteristiques, Objet, Personnage};

pub const CLASSES_VALIDES: [&str; 3] = ["Guerrier", "Mage", "Voleur"];
pub const VALEURS_STANDARD: [u32; 6] = [15, 14, 13, 12, 10, 8];

#[derive(Debug, PartialEq)]
pub enum ErreurCreationPersonnage {
    NomVide,
    ClasseInvalide,
    ValeursCaracteristiquesInvalides,
}

pub fn creer_personnage(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
) -> Result<Personnage, ErreurCreationPersonnage> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALEURS_VALIDES: [u32; 6] = [15, 14, 13, 12, 10, 8];

    #[test]
    fn nom_vide_est_rejete() {
        let resultat = creer_personnage("  ", "Guerrier", VALEURS_VALIDES);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::NomVide));
    }

    #[test]
    fn classe_invalide_est_rejetee() {
        let resultat = creer_personnage("Elowen", "Barde", VALEURS_VALIDES);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ClasseInvalide));
    }

    #[test]
    fn valeurs_hors_tableau_standard_sont_rejetees() {
        let resultat = creer_personnage("Elowen", "Guerrier", [16, 14, 13, 12, 10, 8]);
        assert_eq!(resultat, Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides));
    }

    #[test]
    fn creation_valide_construit_le_personnage_avec_equipement_de_depart() {
        let perso = creer_personnage("Elowen", "Guerrier", VALEURS_VALIDES).unwrap();
        assert_eq!(perso.nom, "Elowen");
        assert_eq!(perso.classe, "Guerrier");
        assert_eq!(perso.caracteristiques.force, 15);
        assert_eq!(perso.caracteristiques.charisme, 8);
        assert_eq!(perso.inventaire.len(), 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application creer_personnage::tests`
Expected: FAIL — compiles, but every test that reaches the `creer_personnage` call panics on `todo!()`.

- [ ] **Step 3: Write the implementation**

Replace the body of `creer_personnage` (keep the constants/tests/error enum above unchanged):

```rust
pub fn creer_personnage(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
) -> Result<Personnage, ErreurCreationPersonnage> {
    if nom.trim().is_empty() {
        return Err(ErreurCreationPersonnage::NomVide);
    }
    if !CLASSES_VALIDES.contains(&classe) {
        return Err(ErreurCreationPersonnage::ClasseInvalide);
    }

    let mut triees = valeurs;
    triees.sort_unstable();
    let mut standard_triees = VALEURS_STANDARD;
    standard_triees.sort_unstable();
    if triees != standard_triees {
        return Err(ErreurCreationPersonnage::ValeursCaracteristiquesInvalides);
    }

    let caracteristiques = Caracteristiques {
        force: valeurs[0],
        dexterite: valeurs[1],
        constitution: valeurs[2],
        intelligence: valeurs[3],
        sagesse: valeurs[4],
        charisme: valeurs[5],
    };

    let inventaire = vec![Objet::epee(), Objet::potion_de_soin()];

    Ok(Personnage::nouveau(
        nom.to_string(),
        classe.to_string(),
        caracteristiques,
        inventaire,
    ))
}
```

Wire the module: `crates/application/src/usecases/mod.rs`:

```rust
pub mod creer_personnage;
```

Update `crates/application/src/lib.rs`:

```rust
pub mod ports;
pub mod usecases;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (8 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases crates/application/src/lib.rs
git commit -m "feat(application): add creer_personnage use case"
```

---

## Task 12: Application — cas d'usage `commencer_combat`

**Files:**
- Create: `crates/application/src/usecases/commencer_combat.rs`
- Modify: `crates/application/src/usecases/mod.rs`

**Interfaces:**
- Consumes: `domain::{determiner_ordre_initiative, modificateur, Combat, De, Ennemi, Personnage}`, `crate::ports::generateur_de::{GenerateurDe, GenerateurDeSeed}` (test only).
- Produces: `application::usecases::commencer_combat::commencer_combat(personnage: &Personnage, ennemis: Vec<Ennemi>, de: &mut dyn GenerateurDe) -> Combat`.

- [ ] **Step 1: Write the failing tests**

```rust
use crate::ports::generateur_de::GenerateurDe;
use domain::{determiner_ordre_initiative, modificateur, Combat, De, Ennemi, Personnage};

pub fn commencer_combat(
    personnage: &Personnage,
    ennemis: Vec<Ennemi>,
    de: &mut dyn GenerateurDe,
) -> Combat {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::generateur_de::GenerateurDeSeed;
    use domain::Caracteristiques;

    fn perso_dex_14() -> Personnage {
        Personnage::nouveau(
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
        )
    }

    fn gobelin() -> Ennemi {
        Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)
    }

    #[test]
    fn joueur_plus_rapide_que_l_ennemi() {
        // joueur: 10 + modificateur(14)=2 => total 12
        // gobelin: 5 + bonus_initiative 2 => total 7
        let mut de = GenerateurDeSeed::nouveau(vec![10, 5]);
        let combat = commencer_combat(&perso_dex_14(), vec![gobelin()], &mut de);
        assert!(!combat.ennemi_agit_avant_joueur);
        assert_eq!(combat.ennemis.len(), 1);
        assert!(!combat.termine);
    }

    #[test]
    fn ennemi_plus_rapide_que_le_joueur() {
        // joueur: 3 + 2 = 5
        // gobelin: 15 + 2 = 17
        let mut de = GenerateurDeSeed::nouveau(vec![3, 15]);
        let combat = commencer_combat(&perso_dex_14(), vec![gobelin()], &mut de);
        assert!(combat.ennemi_agit_avant_joueur);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application commencer_combat::tests`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Write the implementation**

Replace the `todo!()` body:

```rust
pub fn commencer_combat(
    personnage: &Personnage,
    ennemis: Vec<Ennemi>,
    de: &mut dyn GenerateurDe,
) -> Combat {
    let initiative_joueur = de
        .lancer(De { faces: 20 }, modificateur(personnage.caracteristiques.dexterite))
        .total;
    let initiatives_ennemis: Vec<i32> = ennemis
        .iter()
        .map(|e| de.lancer(De { faces: 20 }, e.bonus_initiative).total)
        .collect();

    let ennemi_agit_avant_joueur = determiner_ordre_initiative(initiative_joueur, &initiatives_ennemis);

    Combat {
        ennemis,
        ennemi_agit_avant_joueur,
        tour_actuel: 0,
        termine: false,
        vainqueur: None,
    }
}
```

Update `crates/application/src/usecases/mod.rs`:

```rust
pub mod commencer_combat;
pub mod creer_personnage;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (10 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases/commencer_combat.rs crates/application/src/usecases/mod.rs
git commit -m "feat(application): add commencer_combat use case"
```

---

## Task 13: Application — `valider_action`

**Files:**
- Create: `crates/application/src/usecases/jouer_tour.rs`
- Modify: `crates/application/src/usecases/mod.rs`

**Interfaces:**
- Consumes: `domain::{ActionJoueur, Combat, EffetObjet, EtatPartie}`.
- Produces: `application::usecases::jouer_tour::{valider_action, ErreurTour}` where `valider_action(action: &ActionJoueur, etat: &EtatPartie) -> Result<(), ErreurTour>`.

- [ ] **Step 1: Write the failing tests**

```rust
use domain::{ActionJoueur, EffetObjet, EtatPartie};

#[derive(Debug, PartialEq)]
pub enum ErreurTour {
    AucunCombatEnCours,
    CibleInvalide,
    ObjetInvalide,
    EchecNarrateur,
}

pub fn valider_action(action: &ActionJoueur, etat: &EtatPartie) -> Result<(), ErreurTour> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                vec![Objet::potion_de_soin()],
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
    fn attaquer_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn attaquer_une_cible_inexistante_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Dragon".into() }, &etat_en_combat());
        assert_eq!(resultat, Err(ErreurTour::CibleInvalide));
    }

    #[test]
    fn attaquer_une_cible_vivante_est_valide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn utiliser_un_objet_absent_est_invalide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion inexistante".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_arme_via_utiliser_objet_est_invalide() {
        let mut etat = etat_sans_combat();
        etat.personnage.inventaire.push(Objet::epee());
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Épée".into() }, &etat);
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_potion_presente_est_valide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn fuir_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn fuir_en_combat_est_valide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn action_libre_est_toujours_valide() {
        let resultat = valider_action(&ActionJoueur::ActionLibre("Je regarde autour de moi".into()), &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }
}
```

Note: `EffetObjet` is imported for use in `valider_action`'s implementation (Step 3), not directly in these tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application jouer_tour::tests`
Expected: FAIL — `todo!()` panics on every test.

- [ ] **Step 3: Write the implementation**

Replace the `todo!()` body:

```rust
pub fn valider_action(action: &ActionJoueur, etat: &EtatPartie) -> Result<(), ErreurTour> {
    match action {
        ActionJoueur::Attaquer { cible } => {
            let combat = etat.combat_en_cours.as_ref().ok_or(ErreurTour::AucunCombatEnCours)?;
            let trouve = combat.ennemis.iter().any(|e| &e.nom == cible && !e.est_mort());
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::CibleInvalide)
            }
        }
        ActionJoueur::UtiliserObjet { objet } => {
            let trouve = etat.personnage.inventaire.iter().any(|o| {
                &o.nom == objet && matches!(o.effet, Some(EffetObjet::Consommable { .. }))
            });
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::ObjetInvalide)
            }
        }
        ActionJoueur::Fuir => {
            if etat.combat_en_cours.is_some() {
                Ok(())
            } else {
                Err(ErreurTour::AucunCombatEnCours)
            }
        }
        ActionJoueur::ActionLibre(_) => Ok(()),
    }
}
```

Wire the module: update `crates/application/src/usecases/mod.rs`:

```rust
pub mod commencer_combat;
pub mod creer_personnage;
pub mod jouer_tour;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (19 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases/jouer_tour.rs crates/application/src/usecases/mod.rs
git commit -m "feat(application): add valider_action for JouerTour"
```

---

## Task 14: Application — `resoudre_action_joueur`

**Files:**
- Modify: `crates/application/src/usecases/jouer_tour.rs`

**Interfaces:**
- Consumes: `domain::{resoudre_attaque, ActionJoueur, De, EffetObjet, EtatPartie, Vainqueur}`, `crate::ports::generateur_de::GenerateurDe`.
- Produces: `application::usecases::jouer_tour::resoudre_action_joueur(etat: &mut EtatPartie, action: &ActionJoueur, de: &mut dyn GenerateurDe) -> Vec<String>` — mutates `etat` in place (damages the enemy, heals/consumes items, or ends combat via `Fuir`) and returns human-readable mechanical-result lines.

- [ ] **Step 1: Write the failing tests**

Append to `crates/application/src/usecases/jouer_tour.rs`, above the existing `mod tests` block add the new function signature (body `todo!()` for now), and add these tests inside the existing `#[cfg(test)] mod tests` block (reuse `etat_en_combat()`/`etat_sans_combat()` helpers already there):

```rust
pub fn resoudre_action_joueur(
    etat: &mut EtatPartie,
    action: &ActionJoueur,
    de: &mut dyn GenerateurDe,
) -> Vec<String> {
    todo!()
}
```

```rust
    use crate::ports::generateur_de::GenerateurDeSeed;
    use domain::De;

    #[test]
    fn attaquer_qui_touche_infflige_des_degats() {
        let mut etat = etat_en_combat();
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché)
        // dégâts: d8=5 (épée par défaut quand aucune arme en inventaire -> mains nues d4 ici, voir note)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 4); // 7 - 3
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn attaquer_qui_rate_n_infflige_aucun_degat() {
        let mut etat = etat_en_combat();
        // attaque: d20=5 + mod_dex(14)=2 => 7 < defense 12 (manqué)
        let mut de = GenerateurDeSeed::nouveau(vec![5, 3]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 7);
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn utiliser_objet_soigne_et_retire_l_objet_de_l_inventaire() {
        let mut etat = etat_sans_combat();
        etat.personnage.subir_degats(5);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &mut de);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max);
        assert!(etat.personnage.inventaire.is_empty());
        assert!(resume[0].contains("Potion de soin"));
    }

    #[test]
    fn fuir_termine_le_combat_avec_le_vainqueur_fuite() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        resoudre_action_joueur(&mut etat, &ActionJoueur::Fuir, &mut de);
        let combat = etat.combat_en_cours.as_ref().unwrap();
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Fuite));
    }

    #[test]
    fn action_libre_ne_consomme_aucun_de() {
        let mut etat = etat_sans_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::ActionLibre("J'observe".into()), &mut de);
        assert!(resume.is_empty());
    }
```

Note on the first two tests: `Personnage::nouveau` in `etat_en_combat()`'s underlying `etat_sans_combat()` is built with no weapon in inventory (only a potion), so `arme_equipee()` returns the mains-nues default `De { faces: 4 }`. A roll of `3` on a d4 is valid (max face value is 4), so the damage queue value `3` is consistent with that die.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application jouer_tour::tests`
Expected: FAIL — new tests panic on `todo!()`.

- [ ] **Step 3: Write the implementation**

Replace the `todo!()` body:

```rust
pub fn resoudre_action_joueur(
    etat: &mut EtatPartie,
    action: &ActionJoueur,
    de: &mut dyn GenerateurDe,
) -> Vec<String> {
    let mut resume = Vec::new();
    match action {
        ActionJoueur::Attaquer { cible } => {
            let modificateur_dex = domain::modificateur(etat.personnage.caracteristiques.dexterite);
            let jet_attaque = de.lancer(De { faces: 20 }, modificateur_dex);
            let arme = etat.personnage.arme_equipee();
            let jet_degats = de.lancer(arme, 0);

            if let Some(combat) = etat.combat_en_cours.as_mut() {
                if let Some(ennemi) = combat.trouver_ennemi_vivant_mut(cible) {
                    let resolution = resoudre_attaque(jet_attaque.total, ennemi.defense, jet_degats);
                    if resolution.touche {
                        ennemi.subir_degats(resolution.degats);
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — touché ! Dégâts : {}",
                            ennemi.nom, jet_attaque.total, ennemi.defense, resolution.degats
                        ));
                    } else {
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — manqué",
                            ennemi.nom, jet_attaque.total, ennemi.defense
                        ));
                    }
                }
            }
        }
        ActionJoueur::UtiliserObjet { objet } => {
            if let Some(obj) = etat.personnage.retirer_objet(objet) {
                if let Some(EffetObjet::Consommable { soin }) = obj.effet {
                    etat.personnage.soigner(soin as i32);
                    resume.push(format!("🎲 Tu utilises {} : +{} PV", obj.nom, soin));
                }
            }
        }
        ActionJoueur::Fuir => {
            if let Some(combat) = etat.combat_en_cours.as_mut() {
                combat.terminer(Vainqueur::Fuite);
            }
            resume.push("Tu prends la fuite.".to_string());
        }
        ActionJoueur::ActionLibre(_) => {}
    }
    resume
}
```

Add the necessary imports at the top of the file: `domain::{resoudre_attaque, De, EffetObjet, Vainqueur}` (merge with the existing `use domain::{ActionJoueur, EtatPartie};` line) and `use crate::ports::generateur_de::GenerateurDe;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (24 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases/jouer_tour.rs
git commit -m "feat(application): add resoudre_action_joueur"
```

---

## Task 15: Application — `resoudre_action_ennemis` et `JouerTour::executer`

**Files:**
- Modify: `crates/application/src/usecases/jouer_tour.rs`

**Interfaces:**
- Consumes: everything produced by Tasks 13-14, plus `crate::ports::narrateur::{ContexteTour, Narrateur, ReponseMj}`.
- Produces: `application::usecases::jouer_tour::{resoudre_action_ennemis, JouerTour, ResultatTour}` where `resoudre_action_ennemis(etat: &mut EtatPartie, de: &mut dyn GenerateurDe) -> Vec<String>` and `JouerTour::executer(etat: &mut EtatPartie, saisie: &str, narrateur: &mut dyn Narrateur, de: &mut dyn GenerateurDe, sur_fragment: &mut dyn FnMut(&str)) -> Result<ResultatTour, ErreurTour>`. `ResultatTour { narration: String, resume_mecanique: Vec<String>, combat_termine: bool }`. This is the use case `app` (Task 20) calls once per turn.

- [ ] **Step 1: Write the failing tests**

Add the new signatures near the top of `jouer_tour.rs` (bodies `todo!()`):

```rust
pub struct ResultatTour {
    pub narration: String,
    pub resume_mecanique: Vec<String>,
    pub combat_termine: bool,
}

pub fn resoudre_action_ennemis(etat: &mut EtatPartie, de: &mut dyn GenerateurDe) -> Vec<String> {
    todo!()
}

pub struct JouerTour;

impl JouerTour {
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        todo!()
    }
}
```

Add to `mod tests`, a controllable test double for `Narrateur`:

```rust
    use crate::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};

    struct NarrateurDeTest {
        action: ActionJoueur,
    }

    impl Narrateur for NarrateurDeTest {
        fn raconter(
            &mut self,
            _contexte: &ContexteTour,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<ReponseMj, ErreurNarrateur> {
            sur_fragment("narration de test");
            Ok(ReponseMj {
                narration: "narration de test".to_string(),
                action_proposee: self.action.clone(),
            })
        }
    }

    #[test]
    fn resoudre_action_ennemis_infflige_des_degats_au_joueur() {
        let mut etat = etat_en_combat();
        // ennemi (Gobelin, bonus_attaque 3) : d20=10 => total 13 >= défense joueur 12 (touché)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 4]); // attaque puis dégâts (d6=4)
        let resume = resoudre_action_ennemis(&mut etat, &mut de);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn resoudre_action_ennemis_ne_fait_rien_si_combat_termine() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().terminer(Vainqueur::Joueur);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_ennemis(&mut etat, &mut de);
        assert!(resume.is_empty());
    }

    #[test]
    fn jouer_tour_joueur_tue_l_ennemi_et_termine_le_combat() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemis[0].pv_actuels = 1;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3 (mains nues, tue le gobelin à 1 pv)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert!(resultat.combat_termine);
        assert_eq!(etat.combat_en_cours.as_ref().unwrap().vainqueur, Some(Vainqueur::Joueur));
    }

    #[test]
    fn jouer_tour_ennemi_plus_rapide_agit_avant_le_joueur() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemi_agit_avant_joueur = true;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        // ennemi: d20=10 + bonus_attaque 3 = 13 >= defense joueur 12 (touché) ; dégâts d6=4
        let mut de = GenerateurDeSeed::nouveau(vec![10, 4]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert!(!resultat.combat_termine);
    }

    #[test]
    fn jouer_tour_rejette_une_action_invalide_sans_consommer_de_de() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque", &mut narrateur, &mut de, &mut |_| {});
        assert!(matches!(resultat, Err(ErreurTour::AucunCombatEnCours)));
    }
```

Note: this last test uses `matches!` rather than `assert_eq!` because `ResultatTour` (the `Ok` payload) doesn't derive `PartialEq` — there's no need for it elsewhere, so we avoid adding it just for this one assertion.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application jouer_tour::tests`
Expected: FAIL — new tests panic on `todo!()`.

- [ ] **Step 3: Write the implementation**

```rust
pub fn resoudre_action_ennemis(etat: &mut EtatPartie, de: &mut dyn GenerateurDe) -> Vec<String> {
    let mut resume = Vec::new();
    let defense_joueur = etat.personnage.defense();

    let Some(combat) = etat.combat_en_cours.as_mut() else {
        return resume;
    };
    if combat.termine {
        return resume;
    }

    let mut degats_totaux = 0;
    for ennemi in combat.ennemis.iter_mut().filter(|e| !e.est_mort()) {
        let jet_attaque = de.lancer(De { faces: 20 }, ennemi.bonus_attaque);
        let jet_degats = de.lancer(ennemi.degats, 0);
        let resolution = resoudre_attaque(jet_attaque.total, defense_joueur, jet_degats);
        if resolution.touche {
            degats_totaux += resolution.degats;
            resume.push(format!(
                "🎲 {} t'attaque : {} vs défense {} — touché ! Dégâts : {}",
                ennemi.nom, jet_attaque.total, defense_joueur, resolution.degats
            ));
        } else {
            resume.push(format!(
                "🎲 {} t'attaque : {} vs défense {} — manqué",
                ennemi.nom, jet_attaque.total, defense_joueur
            ));
        }
    }

    etat.personnage.subir_degats(degats_totaux);
    resume
}

fn combat_termine(etat: &EtatPartie) -> bool {
    etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(true)
}

fn terminer_si_ennemis_morts(etat: &mut EtatPartie) {
    if let Some(combat) = etat.combat_en_cours.as_mut() {
        if combat.tous_ennemis_morts() {
            combat.terminer(Vainqueur::Joueur);
        }
    }
}

fn terminer_si_joueur_mort(etat: &mut EtatPartie) {
    if etat.personnage.est_mort() {
        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.terminer(Vainqueur::Ennemis);
        }
    }
}

pub struct JouerTour;

impl JouerTour {
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        let contexte = ContexteTour {
            etat: etat.clone(),
            saisie_joueur: saisie.to_string(),
        };
        let ReponseMj { narration, action_proposee } = narrateur
            .raconter(&contexte, sur_fragment)
            .map_err(|_| ErreurTour::EchecNarrateur)?;

        valider_action(&action_proposee, etat)?;

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

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            combat_termine: combat_termine(etat),
        })
    }
}
```

Add imports at the top of the file: `use crate::ports::narrateur::{ContexteTour, Narrateur, ReponseMj};`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (29 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases/jouer_tour.rs
git commit -m "feat(application): add resoudre_action_ennemis and JouerTour::executer"
```

---

## Task 16: Application — `sauvegarder_partie` / `charger_partie`

**Files:**
- Create: `crates/application/src/usecases/partie.rs`
- Modify: `crates/application/src/usecases/mod.rs`

**Interfaces:**
- Consumes: `crate::ports::sauvegarde::{Sauvegarde, ErreurSauvegarde}`, `domain::EtatPartie`.
- Produces: `application::usecases::partie::{sauvegarder_partie, charger_partie}` — thin pass-throughs used by `app` so it only depends on `application`, not `infra`'s concrete `Sauvegarde` impl.

- [ ] **Step 1: Write the failing tests**

```rust
use crate::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use domain::EtatPartie;

pub fn sauvegarder_partie(sauvegarde: &dyn Sauvegarde, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
    todo!()
}

pub fn charger_partie(sauvegarde: &dyn Sauvegarde) -> Result<EtatPartie, ErreurSauvegarde> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Personnage};
    use std::cell::RefCell;

    struct SauvegardeMemoire {
        contenu: RefCell<Option<EtatPartie>>,
    }

    impl Sauvegarde for SauvegardeMemoire {
        fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
            self.contenu.borrow().clone().ok_or(ErreurSauvegarde::FichierAbsent)
        }

        fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
            *self.contenu.borrow_mut() = Some(etat.clone());
            Ok(())
        }
    }

    fn etat_test() -> EtatPartie {
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

    #[test]
    fn sauvegarder_puis_charger_fait_un_aller_retour() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        let etat = etat_test();
        sauvegarder_partie(&sauvegarde, &etat).unwrap();
        let relu = charger_partie(&sauvegarde).unwrap();
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
    }

    #[test]
    fn charger_sans_sauvegarde_propage_l_erreur() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        assert!(matches!(charger_partie(&sauvegarde), Err(ErreurSauvegarde::FichierAbsent)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p application partie::tests`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Write the implementation**

```rust
pub fn sauvegarder_partie(sauvegarde: &dyn Sauvegarde, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
    sauvegarde.sauvegarder(etat)
}

pub fn charger_partie(sauvegarde: &dyn Sauvegarde) -> Result<EtatPartie, ErreurSauvegarde> {
    sauvegarde.charger()
}
```

Update `crates/application/src/usecases/mod.rs`:

```rust
pub mod commencer_combat;
pub mod creer_personnage;
pub mod jouer_tour;
pub mod partie;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p application`
Expected: PASS (31 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/usecases/partie.rs crates/application/src/usecases/mod.rs
git commit -m "feat(application): add sauvegarder_partie and charger_partie"
```

---

## Task 17: Infra — `FauxNarrateur`

**Files:**
- Create: `crates/infra/src/faux_narrateur.rs`
- Modify: `crates/infra/src/lib.rs` (created empty by Task 1)

**Interfaces:**
- Consumes: `application::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj}`, `domain::{ActionJoueur, EffetObjet}`.
- Produces: `infra::FauxNarrateur`, implementing `Narrateur` by keyword-matching the player's free text.

- [ ] **Step 1: Write the failing tests**

```rust
use application::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};
use domain::{ActionJoueur, EffetObjet};

pub struct FauxNarrateur;

impl Narrateur for FauxNarrateur {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur> {
        todo!()
    }
}

fn trouver_premier_ennemi_vivant(contexte: &ContexteTour) -> Option<String> {
    todo!()
}

fn trouver_premiere_potion(contexte: &ContexteTour) -> Option<String> {
    todo!()
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
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn potion_propose_d_utiliser_la_potion_en_inventaire() {
        let ctx = contexte("Je bois une potion de soin", None);
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn fuite_propose_de_fuir() {
        let ctx = contexte("Je fuis !", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::Fuir);
    }

    #[test]
    fn texte_sans_mot_cle_devient_une_action_libre() {
        let ctx = contexte("J'observe les environs", None);
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::ActionLibre("J'observe les environs".into()));
    }

    #[test]
    fn la_narration_est_streamee_fragment_par_fragment() {
        let ctx = contexte("J'observe", None);
        let mut narrateur = FauxNarrateur;
        let mut fragments = Vec::new();
        let reponse = narrateur.raconter(&ctx, &mut |frag| fragments.push(frag.to_string())).unwrap();
        assert_eq!(fragments.concat(), reponse.narration);
        assert!(fragments.len() > 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra faux_narrateur::tests`
Expected: FAIL — `todo!()` panics; `infra` crate's `lib.rs` doesn't yet exist either.

- [ ] **Step 3: Write the implementation**

```rust
use application::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};
use domain::{ActionJoueur, EffetObjet};

pub struct FauxNarrateur;

impl Narrateur for FauxNarrateur {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur> {
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

        let narration = format!("Tu tentes : \"{}\"...", contexte.saisie_joueur);
        for mot in narration.split_inclusive(' ') {
            sur_fragment(mot);
        }

        Ok(ReponseMj { narration, action_proposee: action })
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
```

`crates/infra/src/lib.rs`:

```rust
mod faux_narrateur;

pub use faux_narrateur::FauxNarrateur;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/infra/src/faux_narrateur.rs crates/infra/src/lib.rs
git commit -m "feat(infra): add FauxNarrateur for the console prototype"
```

---

## Task 18: Infra — `GenerateurDeOs`

**Files:**
- Create: `crates/infra/src/generateur_de_os.rs`
- Modify: `crates/infra/src/lib.rs`

**Interfaces:**
- Consumes: `application::ports::generateur_de::GenerateurDe`, `domain::{De, JetDe}`, `rand::Rng`.
- Produces: `infra::GenerateurDeOs`, a real (non-deterministic) implementation of `GenerateurDe`.

- [ ] **Step 1: Write the failing test**

```rust
use application::ports::generateur_de::GenerateurDe;
use domain::{De, JetDe};
use rand::Rng;

pub struct GenerateurDeOs;

impl GenerateurDe for GenerateurDeOs {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lancer_reste_dans_les_bornes_du_de() {
        let mut generateur = GenerateurDeOs;
        for _ in 0..1000 {
            let jet = generateur.lancer(De { faces: 6 }, 0);
            assert!(jet.resultat >= 1 && jet.resultat <= 6, "résultat hors bornes: {}", jet.resultat);
        }
    }

    #[test]
    fn lancer_applique_le_modificateur_au_total() {
        let mut generateur = GenerateurDeOs;
        let jet = generateur.lancer(De { faces: 20 }, 5);
        assert_eq!(jet.total, jet.resultat as i32 + 5);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra generateur_de_os::tests`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Write the implementation**

```rust
impl GenerateurDe for GenerateurDeOs {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe {
        let resultat = rand::thread_rng().gen_range(1..=de.faces);
        JetDe::nouveau(resultat, modificateur)
    }
}
```

Update `crates/infra/src/lib.rs`:

```rust
mod faux_narrateur;
mod generateur_de_os;

pub use faux_narrateur::FauxNarrateur;
pub use generateur_de_os::GenerateurDeOs;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (7 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/infra/src/generateur_de_os.rs crates/infra/src/lib.rs
git commit -m "feat(infra): add GenerateurDeOs backed by rand"
```

---

## Task 19: Infra — `SauvegardeFichier`

**Files:**
- Create: `crates/infra/src/sauvegarde_fichier.rs`
- Modify: `crates/infra/src/lib.rs`

**Interfaces:**
- Consumes: `application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde}`, `domain::EtatPartie`.
- Produces: `infra::SauvegardeFichier`, with `SauvegardeFichier::nouveau(chemin: PathBuf) -> SauvegardeFichier`, implementing `Sauvegarde` by reading/writing a JSON file.

- [ ] **Step 1: Write the failing tests**

```rust
use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use domain::EtatPartie;
use std::fs;
use std::path::PathBuf;

pub struct SauvegardeFichier {
    chemin: PathBuf,
}

impl SauvegardeFichier {
    pub fn nouveau(chemin: PathBuf) -> Self {
        Self { chemin }
    }
}

impl Sauvegarde for SauvegardeFichier {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
        todo!()
    }

    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Personnage};

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("narrateur_test_{}_{}.json", std::process::id(), nom))
    }

    fn etat_test() -> EtatPartie {
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

    #[test]
    fn charger_sans_fichier_retourne_fichier_absent() {
        let chemin = chemin_temporaire("absent");
        let sauvegarde = SauvegardeFichier::nouveau(chemin);
        assert!(matches!(sauvegarde.charger(), Err(ErreurSauvegarde::FichierAbsent)));
    }

    #[test]
    fn sauvegarder_puis_charger_fait_un_aller_retour() {
        let chemin = chemin_temporaire("roundtrip");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let etat = etat_test();

        sauvegarde.sauvegarder(&etat).expect("sauvegarde échouée");
        let relu = sauvegarde.charger().expect("chargement échoué");

        assert_eq!(relu.personnage.nom, etat.personnage.nom);
        assert_eq!(relu.lieu_actuel, etat.lieu_actuel);

        let _ = fs::remove_file(chemin);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p infra sauvegarde_fichier::tests`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Write the implementation**

```rust
impl Sauvegarde for SauvegardeFichier {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
        if !self.chemin.exists() {
            return Err(ErreurSauvegarde::FichierAbsent);
        }
        let contenu = fs::read_to_string(&self.chemin).map_err(|e| ErreurSauvegarde::Lecture(e.to_string()))?;
        serde_json::from_str(&contenu).map_err(|e| ErreurSauvegarde::Lecture(e.to_string()))
    }

    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
        let contenu = serde_json::to_string_pretty(etat).map_err(|e| ErreurSauvegarde::Ecriture(e.to_string()))?;
        fs::write(&self.chemin, contenu).map_err(|e| ErreurSauvegarde::Ecriture(e.to_string()))
    }
}
```

Update `crates/infra/src/lib.rs`:

```rust
mod faux_narrateur;
mod generateur_de_os;
mod sauvegarde_fichier;

pub use faux_narrateur::FauxNarrateur;
pub use generateur_de_os::GenerateurDeOs;
pub use sauvegarde_fichier::SauvegardeFichier;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p infra`
Expected: PASS (9 tests total).

- [ ] **Step 5: Commit**

```bash
git add crates/infra/src/sauvegarde_fichier.rs crates/infra/src/lib.rs
git commit -m "feat(infra): add SauvegardeFichier backed by local JSON"
```

---

## Task 20: App — boucle CLI

**Files:**
- Create: `crates/app/src/creation.rs`
- Modify: `crates/app/src/main.rs`

**Interfaces:**
- Consumes: everything produced by Tasks 11-19.
- Produces: a runnable binary (`cargo run -p app`) that plays a full session: load-or-create character, fixed gobelin encounter, turn loop, save after each turn, exits on combat resolution.

This task has no automated test (per the spec: "pas de test automatisé pour la boucle CLI — I/O interactive — validation manuelle"). Its verification step is a scripted manual playthrough with concrete expected output, run below.

- [ ] **Step 1: Write `crates/app/src/creation.rs`**

```rust
use application::usecases::creer_personnage::{creer_personnage, CLASSES_VALIDES, VALEURS_STANDARD};
use domain::Personnage;
use std::io::{self, Write};

pub fn creer_personnage_interactif() -> Personnage {
    println!("=== Création de personnage ===");

    let nom = lire_ligne_non_vide("Nom de ton personnage : ");
    let classe = lire_classe();
    let valeurs = lire_caracteristiques();

    creer_personnage(&nom, &classe, valeurs).expect("construction du personnage échouée")
}

fn lire_ligne_non_vide(invite: &str) -> String {
    loop {
        print!("{}", invite);
        io::stdout().flush().ok();
        let mut ligne = String::new();
        io::stdin().read_line(&mut ligne).expect("lecture échouée");
        let ligne = ligne.trim().to_string();
        if !ligne.is_empty() {
            return ligne;
        }
        println!("Le nom ne peut pas être vide.");
    }
}

fn lire_classe() -> String {
    loop {
        println!("Classes disponibles : {}", CLASSES_VALIDES.join(", "));
        print!("Choisis une classe : ");
        io::stdout().flush().ok();
        let mut ligne = String::new();
        io::stdin().read_line(&mut ligne).expect("lecture échouée");
        let choix = ligne.trim().to_string();
        if CLASSES_VALIDES.contains(&choix.as_str()) {
            return choix;
        }
        println!("Classe invalide.");
    }
}

fn lire_caracteristiques() -> [u32; 6] {
    let noms = ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];
    let mut disponibles: Vec<u32> = VALEURS_STANDARD.to_vec();
    let mut valeurs = [0u32; 6];

    for (i, nom) in noms.iter().enumerate() {
        loop {
            println!("Valeurs restantes : {:?}", disponibles);
            print!("Valeur pour {} : ", nom);
            io::stdout().flush().ok();
            let mut ligne = String::new();
            io::stdin().read_line(&mut ligne).expect("lecture échouée");
            match ligne.trim().parse::<u32>() {
                Ok(valeur) if disponibles.contains(&valeur) => {
                    disponibles.retain(|&v| v != valeur);
                    valeurs[i] = valeur;
                    break;
                }
                _ => println!("Choisis une valeur parmi celles restantes."),
            }
        }
    }

    valeurs
}
```

- [ ] **Step 2: Write `crates/app/src/main.rs`**

```rust
mod creation;

use application::usecases::commencer_combat::commencer_combat;
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, Ennemi, EtatPartie, Vainqueur};
use infra::{FauxNarrateur, GenerateurDeOs, SauvegardeFichier};
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    let sauvegarde = SauvegardeFichier::nouveau(PathBuf::from("partie.json"));
    let mut de = GenerateurDeOs;
    let mut narrateur = FauxNarrateur;

    let mut etat: EtatPartie = match charger_partie(&sauvegarde) {
        Ok(etat) => {
            println!("Partie reprise pour {}.", etat.personnage.nom);
            etat
        }
        Err(_) => {
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
    };

    loop {
        println!("\nQue fais-tu ?");
        print!("> ");
        io::stdout().flush().ok();
        let mut saisie = String::new();
        io::stdin().read_line(&mut saisie).expect("lecture échouée");
        let saisie = saisie.trim();

        let resultat = JouerTour::executer(&mut etat, saisie, &mut narrateur, &mut de, &mut |frag: &str| {
            print!("{}", frag);
            io::stdout().flush().ok();
        });

        match resultat {
            Ok(resultat) => {
                println!();
                for ligne in &resultat.resume_mecanique {
                    println!("{}", ligne);
                }
                sauvegarder_partie(&sauvegarde, &etat).expect("sauvegarde échouée");

                if resultat.combat_termine {
                    match etat.combat_en_cours.as_ref().and_then(|c| c.vainqueur) {
                        Some(Vainqueur::Joueur) => println!("\nVictoire ! Le gobelin est vaincu."),
                        Some(Vainqueur::Ennemis) => println!("\nTu es tombé au combat..."),
                        Some(Vainqueur::Fuite) => println!("\nTu as fui le combat."),
                        None => {}
                    }
                    break;
                }
            }
            Err(erreur) => {
                println!("Action invalide : {:?}", erreur);
            }
        }
    }
}
```

- [ ] **Step 3: Build the binary**

Run: `cargo build -p app`
Expected: Compiles with no errors.

- [ ] **Step 4: Manual playthrough — fresh game**

From the workspace root, remove any stale save and run the binary:

```bash
rm -f partie.json
cargo run -p app
```

Follow this exact script and confirm the listed expectations:

1. At `Nom de ton personnage :`, type `Elowen` and press Enter. Expected: prompt moves to class selection.
2. At `Choisis une classe :`, type `Guerrier`. Expected: prompt moves to "Valeurs restantes : [15, 14, 13, 12, 10, 8]" / "Valeur pour Force :".
3. Enter the six values in order `15 14 13 12 10 8` (one per prompt, pressing Enter after each). Expected: after the sixth value, the program prints "Que fais-tu ?" and a `>` prompt — character creation is done, combat has started (the gobelin fight begins immediately as the fixed encounter).
4. At `>`, type `j'attaque le gobelin`. Expected: narration streams out character-by-character (or word-by-word), followed by a line starting with `🎲` that says either "touché" or "manqué", then the loop prompts "Que fais-tu ?" again (unless the gobelin died — see step 5).
5. Repeat step 4 (`j'attaque le gobelin`) until either:
   - "Victoire ! Le gobelin est vaincu." is printed and the program exits — confirm `partie.json` exists in the workspace root (`ls partie.json`).
   - "Tu es tombé au combat..." is printed and the program exits (if the gobelin's attacks reduced your PV to 0 first).

- [ ] **Step 5: Manual playthrough — resume from save**

If the game ended in victory or defeat in Step 4, that playthrough is over (no state to resume — the loop already exited). To test resume specifically, interrupt a fresh game mid-combat instead:

```bash
rm -f partie.json
cargo run -p app
```

Go through character creation again (same inputs as Step 4.1-4.3), then at the first `>` prompt type `j'observe les environs` (an `ActionLibre` that doesn't end combat) and press Enter once, then kill the process with `Ctrl+C`. Expected: `partie.json` exists and contains the in-progress combat state (`cat partie.json` shows `"combat_en_cours": { ... "termine": false ... }`).

Run again:

```bash
cargo run -p app
```

Expected: prints `Partie reprise pour Elowen.` (skipping character creation entirely) and goes straight to the `Que fais-tu ?` prompt with the same combat state as before the interrupt.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/creation.rs crates/app/src/main.rs
git commit -m "feat(app): wire the CLI game loop for the console prototype"
```

---

## Completion check

After Task 20:

- `cargo test` (from the workspace root) passes with 0 failures across all 4 crates.
- `cargo build` and `cargo build --release` both succeed.
- The manual playthrough in Task 20 Steps 4-5 has been performed and matched the expected output.
