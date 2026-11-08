# Histoires multiples et choix du modèle — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remplacer la sauvegarde unique (`partie.json`) et le modèle unique (`NARRATEUR_MODELE`/un seul téléchargement Android) par plusieurs histoires persistantes, chacune avec son propre modèle choisi dans un petit catalogue, téléchargé à la demande sur les deux plateformes.

**Architecture:** Deux nouveaux modules autonomes (`modeles.rs` : catalogue de modèles ; `histoires.rs` : index des histoires) puis une réécriture de `worker::lancer()` en boucle à deux niveaux : une boucle externe qui attend le choix d'une histoire (existante ou nouvelle) avant de charger quoi que ce soit, et une boucle interne (`jouer_histoire`) qui traite les tours d'une histoire déjà chargée jusqu'à un retour explicite à la sélection. `domain`/`application`/`crates/app` (console) ne changent pas.

**Tech Stack:** Rust, Dioxus 0.7 (`app_desktop`), `serde`/`serde_json` (nouvelle dépendance directe pour `app_desktop`, déjà utilisée par `domain`/`infra`).

**Spec:** `docs/superpowers/specs/2026-11-08-histoires-multiples-et-modeles-design.md`

## Global Constraints

- Catalogue de modèles (vérifié par HTTP réel, voir le spec) :
  - Léger : `bartowski/Qwen2.5-1.5B-Instruct-GGUF` / `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf`, 986 048 768 octets.
  - Équilibré : `bartowski/Qwen2.5-3B-Instruct-GGUF` / `Qwen2.5-3B-Instruct-Q4_K_M.gguf`, 1 929 903 264 octets.
  - Costaud : `bartowski/Qwen2.5-7B-Instruct-GGUF` / `Qwen2.5-7B-Instruct-Q4_K_M.gguf`, 4 683 074 240 octets.
- `domain`, `application`, `crates/app` (console) ne sont pas modifiés par ce plan.
- Pas de migration de l'ancien `partie.json` unique.
- Les deux plateformes (desktop, Android) partagent exactement le même système de catalogue/téléchargement — aucun palier réservé à une seule plateforme.
- Sur échec de téléchargement ou de chargement du modèle **après** l'écran de sélection (pas seulement au premier lancement), le thread de jeu doit revenir à la sélection des histoires plutôt que se terminer (décision explicite du spec, section « Récupération sur échec »).
- Les modèles téléchargés sont stockés dans un répertoire **partagé** (`repertoire_modeles`), pas un par histoire.

## Review Focus

- Deux histoires créées avec le **même nom de personnage** : leurs ids générés doivent rester distincts.
- L'utilisateur **annule** la confirmation de téléchargement d'une histoire tout juste créée : aucune entrée fantôme ne doit rester dans l'index (l'histoire ne doit être enregistrée qu'une fois son modèle effectivement chargé).
- Reprendre une histoire dont le fichier `<id>.json` est **absent ou corrompu** (modifié/supprimé manuellement) : erreur propre, retour à la sélection, pas de panique du thread.
- Deux histoires différentes choisissant **le même palier** : le second chargement ne doit pas re-télécharger un modèle déjà présent (fichier partagé).
- La **mort du personnage** retire son histoire de l'index et son fichier, **sans toucher aux autres histoires** présentes dans l'index.

---

### Task 1: Catalogue de modèles (`modeles.rs`)

**Files:**
- Create: `crates/app_desktop/src/modeles.rs`
- Modify: `crates/app_desktop/src/main.rs` (ajouter `mod modeles;`)
- Modify: `crates/app_desktop/Cargo.toml` (ajouter `serde`/`serde_json`)

**Interfaces:**
- Produces: `modeles::PaletteModele { Leger, Equilibre, Costaud }` (derive `Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize`, `#[serde(rename_all = "snake_case")]`) ; `PaletteModele::TOUTES: [PaletteModele; 3]` ; méthodes `nom_affiche(&self) -> &'static str`, `nom_fichier(&self) -> &'static str`, `url(&self) -> &'static str`, `taille_approx_octets(&self) -> u64`, `avertissement(&self) -> Option<&'static str>`, `chemin_dans(&self, repertoire_modeles: &std::path::Path) -> std::path::PathBuf`.

- [ ] **Step 1: Ajouter les dépendances**

Dans `crates/app_desktop/Cargo.toml`, section `[dependencies]`, ajouter (même version que `domain`) :

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Écrire les tests (qui doivent échouer, le module n'existe pas)**

Créer `crates/app_desktop/src/modeles.rs` avec seulement :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_palette_a_un_nom_affiche_non_vide() {
        for palette in PaletteModele::TOUTES {
            assert!(!palette.nom_affiche().is_empty());
        }
    }

    #[test]
    fn chaque_palette_a_une_url_huggingface_qui_pointe_vers_son_fichier() {
        for palette in PaletteModele::TOUTES {
            assert!(palette.url().starts_with("https://huggingface.co/"));
            assert!(palette.nom_fichier().ends_with(".gguf"));
            assert!(palette.url().ends_with(palette.nom_fichier()));
        }
    }

    #[test]
    fn seul_le_palier_costaud_a_un_avertissement() {
        assert_eq!(PaletteModele::Leger.avertissement(), None);
        assert_eq!(PaletteModele::Equilibre.avertissement(), None);
        assert!(PaletteModele::Costaud.avertissement().is_some());
    }

    #[test]
    fn la_palette_equilibre_reprend_exactement_le_modele_actuel() {
        assert_eq!(
            PaletteModele::Equilibre.url(),
            "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf"
        );
        assert_eq!(PaletteModele::Equilibre.taille_approx_octets(), 1_929_903_264);
    }

    #[test]
    fn chemin_dans_joint_le_repertoire_et_le_nom_de_fichier() {
        let repertoire = std::path::PathBuf::from("modeles");
        assert_eq!(
            PaletteModele::Equilibre.chemin_dans(&repertoire),
            repertoire.join("Qwen2.5-3B-Instruct-Q4_K_M.gguf")
        );
    }

    #[test]
    fn deux_palettes_differentes_partagent_le_meme_repertoire_mais_pas_le_meme_fichier() {
        let repertoire = std::path::PathBuf::from("modeles");
        assert_ne!(
            PaletteModele::Leger.chemin_dans(&repertoire),
            PaletteModele::Costaud.chemin_dans(&repertoire)
        );
    }
}
```

- [ ] **Step 3: Vérifier que les tests échouent à la compilation**

Run: `cargo test -p app_desktop modeles`
Expected: FAIL (compilation) — `PaletteModele` n'existe pas.

- [ ] **Step 4: Implémenter `PaletteModele`**

Ajouter avant le `#[cfg(test)]` :

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteModele {
    Leger,
    Equilibre,
    Costaud,
}

impl PaletteModele {
    pub const TOUTES: [PaletteModele; 3] = [PaletteModele::Leger, PaletteModele::Equilibre, PaletteModele::Costaud];

    pub fn nom_affiche(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "Léger",
            PaletteModele::Equilibre => "Équilibré",
            PaletteModele::Costaud => "Costaud",
        }
    }

    pub fn nom_fichier(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            PaletteModele::Equilibre => "Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            PaletteModele::Costaud => "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn url(&self) -> &'static str {
        match self {
            PaletteModele::Leger => "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            PaletteModele::Equilibre => "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            PaletteModele::Costaud => "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn taille_approx_octets(&self) -> u64 {
        match self {
            PaletteModele::Leger => 986_048_768,
            PaletteModele::Equilibre => 1_929_903_264,
            PaletteModele::Costaud => 4_683_074_240,
        }
    }

    pub fn avertissement(&self) -> Option<&'static str> {
        match self {
            PaletteModele::Costaud => Some("Plus lent et plus gourmand en mémoire, surtout sur mobile."),
            _ => None,
        }
    }

    pub fn chemin_dans(&self, repertoire_modeles: &std::path::Path) -> std::path::PathBuf {
        repertoire_modeles.join(self.nom_fichier())
    }
}
```

- [ ] **Step 5: Déclarer le module et vérifier que les tests passent**

Ajouter `mod modeles;` dans `crates/app_desktop/src/main.rs`, à côté des autres `mod`.

Run: `cargo test -p app_desktop modeles`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/app_desktop/Cargo.toml crates/app_desktop/src/modeles.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add a model catalog (Leger/Equilibre/Costaud)"
```

---

### Task 2: Index des histoires (`histoires.rs`)

**Files:**
- Create: `crates/app_desktop/src/histoires.rs`
- Modify: `crates/app_desktop/src/main.rs` (ajouter `mod histoires;`)

**Interfaces:**
- Consumes: `modeles::PaletteModele` (Task 1).
- Produces: `histoires::InfoHistoire { id: String, nom_personnage: String, classe: String, palette_modele: PaletteModele, cree_le: u128 }` (derive `Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize`) ; `histoires::lister(repertoire_sauvegardes: &Path) -> Vec<InfoHistoire>` (triée par `cree_le` décroissant) ; `histoires::generer_id(nom_personnage: &str) -> String` ; `histoires::ajouter(repertoire_sauvegardes: &Path, entree: InfoHistoire) -> std::io::Result<()>` ; `histoires::retirer(repertoire_sauvegardes: &Path, id: &str) -> std::io::Result<()>` (retire uniquement l'entrée d'index — ne supprime PAS `<id>.json`, à la charge de l'appelant, même principe que l'actuel `std::fs::remove_file` appelé directement dans `worker.rs`) ; `histoires::chemin_sauvegarde(repertoire_sauvegardes: &Path, id: &str) -> PathBuf`.

- [ ] **Step 1: Écrire les tests (qui doivent échouer, le module n'existe pas)**

Créer `crates/app_desktop/src/histoires.rs` avec seulement :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modeles::PaletteModele;
    use std::path::PathBuf;

    fn repertoire_temporaire(nom: &str) -> PathBuf {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_histoires_test_{}_{}",
            std::process::id(),
            nom
        ));
        let _ = std::fs::remove_dir_all(&chemin);
        chemin
    }

    fn entree_test(id: &str, nom: &str) -> InfoHistoire {
        InfoHistoire {
            id: id.to_string(),
            nom_personnage: nom.to_string(),
            classe: "Guerrier".to_string(),
            palette_modele: PaletteModele::Equilibre,
            cree_le: 1,
        }
    }

    #[test]
    fn lister_sans_index_retourne_une_liste_vide() {
        let repertoire = repertoire_temporaire("vide");
        assert_eq!(lister(&repertoire), Vec::new());
    }

    #[test]
    fn ajouter_puis_lister_retrouve_l_entree() {
        let repertoire = repertoire_temporaire("ajout");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        let histoires = lister(&repertoire);
        assert_eq!(histoires.len(), 1);
        assert_eq!(histoires[0].id, "elowen-abc123");
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn retirer_enleve_seulement_l_entree_visee() {
        let repertoire = repertoire_temporaire("retrait");
        ajouter(&repertoire, entree_test("elowen-abc123", "Elowen")).unwrap();
        ajouter(&repertoire, entree_test("thorn-def456", "Thorn")).unwrap();
        retirer(&repertoire, "elowen-abc123").unwrap();
        let histoires = lister(&repertoire);
        assert_eq!(histoires.len(), 1, "l'autre histoire ne doit pas disparaître");
        assert_eq!(histoires[0].id, "thorn-def456");
        let _ = std::fs::remove_dir_all(&repertoire);
    }

    #[test]
    fn generer_id_reste_distinct_meme_pour_le_meme_nom() {
        let id1 = generer_id("Elowen");
        let id2 = generer_id("Elowen");
        assert_ne!(id1, id2, "même nom, deux appels : ids tout de même distincts");
        assert!(id1.starts_with("elowen-"));
    }

    #[test]
    fn chemin_sauvegarde_pointe_vers_id_json() {
        let repertoire = PathBuf::from("sauvegardes");
        assert_eq!(chemin_sauvegarde(&repertoire, "elowen-abc123"), repertoire.join("elowen-abc123.json"));
    }
}
```

- [ ] **Step 2: Vérifier que les tests échouent à la compilation**

Run: `cargo test -p app_desktop histoires`
Expected: FAIL (compilation) — rien de tout ça n'existe encore.

- [ ] **Step 3: Implémenter le module**

Ajouter avant le `#[cfg(test)]` :

```rust
use crate::modeles::PaletteModele;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InfoHistoire {
    pub id: String,
    pub nom_personnage: String,
    pub classe: String,
    pub palette_modele: PaletteModele,
    /// Nanosecondes depuis l'epoch Unix — sert uniquement à trier par
    /// ordre de création, pas affiché tel quel.
    pub cree_le: u128,
}

const NOM_INDEX: &str = "index.json";

/// Liste les histoires existantes, les plus récemment créées en premier.
/// Pas de répertoire/index = aucune histoire pour l'instant (premier
/// lancement) : retourne une liste vide, ce n'est pas une erreur.
pub fn lister(repertoire_sauvegardes: &Path) -> Vec<InfoHistoire> {
    let chemin_index = repertoire_sauvegardes.join(NOM_INDEX);
    let Ok(contenu) = std::fs::read_to_string(&chemin_index) else {
        return Vec::new();
    };
    let mut histoires: Vec<InfoHistoire> = serde_json::from_str(&contenu).unwrap_or_default();
    histoires.sort_by(|a, b| b.cree_le.cmp(&a.cree_le));
    histoires
}

/// Id interne (jamais affiché) : nom normalisé (minuscules, alphanumérique
/// uniquement) + suffixe temporel pour l'unicité, y compris entre deux
/// histoires du même nom.
pub fn generer_id(nom_personnage: &str) -> String {
    let normalise: String = nom_personnage
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let horodatage = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{normalise}-{horodatage:x}")
}

pub fn ajouter(repertoire_sauvegardes: &Path, entree: InfoHistoire) -> std::io::Result<()> {
    std::fs::create_dir_all(repertoire_sauvegardes)?;
    let mut histoires = lister(repertoire_sauvegardes);
    histoires.push(entree);
    ecrire_index(repertoire_sauvegardes, &histoires)
}

pub fn retirer(repertoire_sauvegardes: &Path, id: &str) -> std::io::Result<()> {
    let mut histoires = lister(repertoire_sauvegardes);
    histoires.retain(|h| h.id != id);
    ecrire_index(repertoire_sauvegardes, &histoires)
}

fn ecrire_index(repertoire_sauvegardes: &Path, histoires: &[InfoHistoire]) -> std::io::Result<()> {
    let chemin_index = repertoire_sauvegardes.join(NOM_INDEX);
    let contenu = serde_json::to_string_pretty(histoires).expect("sérialisation de l'index");
    std::fs::write(&chemin_index, contenu)
}

pub fn chemin_sauvegarde(repertoire_sauvegardes: &Path, id: &str) -> PathBuf {
    repertoire_sauvegardes.join(format!("{id}.json"))
}
```

- [ ] **Step 4: Déclarer le module et vérifier que les tests passent**

Ajouter `mod histoires;` dans `crates/app_desktop/src/main.rs`.

Run: `cargo test -p app_desktop histoires`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/app_desktop/src/histoires.rs crates/app_desktop/src/main.rs
git commit -m "feat(app_desktop): add a story index (list/add/remove, replaces the single save file)"
```

---

### Task 3: Protocole, téléchargement généralisé, et réécriture de `worker::lancer()`

C'est la tâche centrale : `protocole.rs`, `telechargement.rs` et `worker.rs` changent ensemble parce que Rust impose l'exhaustivité des `match` sur les énumérations du protocole — les séparer laisserait le crate dans un état qui ne compile pas entre deux tâches.

**Files:**
- Modify: `crates/app_desktop/src/protocole.rs` (réécriture complète)
- Modify: `crates/app_desktop/src/telechargement.rs`
- Modify: `crates/app_desktop/src/worker.rs` (réécriture complète)

**Interfaces:**
- Consumes: `modeles::PaletteModele` (Task 1), `histoires::{InfoHistoire, lister, generer_id, ajouter, retirer, chemin_sauvegarde}` (Task 2).
- Produces :
  - `protocole::MessageVersWorker { ReprendreHistoire(String), CreerPersonnage { nom, classe, valeurs, palette_modele: PaletteModele }, JouerTour(String), RetourSelection, ConfirmerTelechargement(bool) }`
  - `protocole::MessageVersUi { ListeHistoires(Vec<InfoHistoire>), PartieCreee(EtatPartie), NarrationFragment(String), TourTermine { etat, resultat_narration, resume_mecanique, vainqueur_combat, partie_terminee }, TourEchoue(String), ProgressionTelechargement(f32), ConfirmationTelechargementRequise { palette: PaletteModele, taille_octets: u64 } }`
  - `protocole::Vue { SelectionHistoire(Vec<InfoHistoire>), Creation, Jeu, FinDePartie(Option<Vainqueur>), ErreurChargement(String), Telechargement { palette: PaletteModele, progression: f32 }, ConfirmationTelechargement { palette: PaletteModele, taille_octets: u64 } }`
  - `telechargement::telecharger_modele(palette: &PaletteModele, chemin_destination: &Path, sur_progression: &mut dyn FnMut(f32)) -> Result<(), ErreurTelechargement>`
  - `telechargement::verifier_taille_modele(palette: &PaletteModele) -> Result<u64, ErreurTelechargement>`
  - `worker::repertoire_sauvegardes_par_defaut() -> PathBuf`, `worker::repertoire_modeles_par_defaut() -> PathBuf`
  - `worker::creer_nouvelle_histoire(nom: &str, classe: &str, valeurs: [u32; 6], de: &mut dyn GenerateurDe) -> Result<EtatPartie, ErreurCreationPersonnage>`
  - `worker::lancer(rx, tx, repertoire_sauvegardes: PathBuf, repertoire_modeles: PathBuf)`

#### Étape A — `protocole.rs`

- [ ] **Step 1: Remplacer tout le contenu de `protocole.rs`**

```rust
use domain::{EtatPartie, Vainqueur};

pub use crate::histoires::InfoHistoire;
pub use crate::modeles::PaletteModele;

#[derive(Debug, Clone)]
pub enum MessageVersWorker {
    ReprendreHistoire(String),
    CreerPersonnage { nom: String, classe: String, valeurs: [u32; 6], palette_modele: PaletteModele },
    JouerTour(String),
    RetourSelection,
    ConfirmerTelechargement(bool),
}

#[derive(Debug, Clone)]
pub enum MessageVersUi {
    ListeHistoires(Vec<InfoHistoire>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat_narration: String, resume_mecanique: Vec<String>, vainqueur_combat: Option<Vainqueur>, partie_terminee: bool },
    TourEchoue(String),
    ProgressionTelechargement(f32),
    ConfirmationTelechargementRequise { palette: PaletteModele, taille_octets: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vue {
    SelectionHistoire(Vec<InfoHistoire>),
    Creation,
    Jeu,
    FinDePartie(Option<Vainqueur>),
    ErreurChargement(String),
    Telechargement { palette: PaletteModele, progression: f32 },
    ConfirmationTelechargement { palette: PaletteModele, taille_octets: u64 },
}
```

(Les anciens `EtatInitial`/`PartieReinitialisee` disparaissent : `ListeHistoires` les remplace tous les deux — au démarrage comme après un retour à la sélection, c'est le même message.)

- [ ] **Step 2: Vérifier que `cargo check -p app_desktop` échoue**

Run: `cargo check -p app_desktop`
Expected: FAIL — `telechargement.rs`/`worker.rs`/`main.rs` référencent encore les anciennes formes. C'est attendu, les étapes suivantes corrigent ça.

#### Étape B — `telechargement.rs`

- [ ] **Step 3: Généraliser les deux fonctions publiques**

Dans `crates/app_desktop/src/telechargement.rs`, supprimer la constante `MODELE_URL` et remplacer :

```rust
pub fn telecharger_modele(
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    telecharger_depuis(MODELE_URL, chemin_destination, sur_progression)
}

pub fn verifier_taille_modele() -> Result<u64, ErreurTelechargement> {
    verifier_taille_depuis(MODELE_URL)
}
```

par :

```rust
pub fn telecharger_modele(
    palette: &crate::modeles::PaletteModele,
    chemin_destination: &Path,
    sur_progression: &mut dyn FnMut(f32),
) -> Result<(), ErreurTelechargement> {
    telecharger_depuis(palette.url(), chemin_destination, sur_progression)
}

pub fn verifier_taille_modele(palette: &crate::modeles::PaletteModele) -> Result<u64, ErreurTelechargement> {
    verifier_taille_depuis(palette.url())
}
```

`telecharger_depuis`/`verifier_taille_depuis` (déjà paramétrées par une URL) et tous leurs tests existants restent inchangés.

- [ ] **Step 4: Vérifier que les tests de ce fichier passent toujours**

Run: `cargo test -p app_desktop telechargement`
Expected: PASS (les mêmes tests qu'avant, inchangés — `telecharger_depuis`/`verifier_taille_depuis` ne sont pas touchées).

#### Étape C — `worker.rs`

- [ ] **Step 5: Remplacer tout le contenu de `worker.rs`**

```rust
use application::ports::generateur_de::GenerateurDe;
use application::ports::narrateur::Narrateur;
use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use application::usecases::commencer_combat::commencer_combat;
use application::usecases::creer_personnage::{creer_personnage, ErreurCreationPersonnage};
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, Ennemi, EtatPartie};
use std::path::{Path, PathBuf};

use crate::histoires::{self, InfoHistoire};
use crate::protocole::{MessageVersUi, MessageVersWorker};

/// Répertoire de stockage privé de l'app sur Android — inchangé depuis le
/// portage Android (JNI `Activity.getFilesDir()`), voir le commentaire
/// historique si besoin de le retoucher. Partagé entre les deux fonctions
/// de répertoire ci-dessous.
#[cfg(feature = "plateforme-android")]
fn repertoire_donnees_app() -> PathBuf {
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
    let chemin_jstring = jni::objects::JString::from(chemin_jstring);
    let chemin_java_str = env
        .get_string(&chemin_jstring)
        .expect("conversion JString -> JavaStr");
    PathBuf::from(chemin_java_str.to_str().expect("UTF-8 valide"))
}

#[cfg(feature = "plateforme-desktop")]
pub fn repertoire_sauvegardes_par_defaut() -> PathBuf {
    PathBuf::from("sauvegardes")
}

#[cfg(feature = "plateforme-android")]
pub fn repertoire_sauvegardes_par_defaut() -> PathBuf {
    repertoire_donnees_app().join("sauvegardes")
}

#[cfg(feature = "plateforme-desktop")]
pub fn repertoire_modeles_par_defaut() -> PathBuf {
    PathBuf::from("modeles")
}

#[cfg(feature = "plateforme-android")]
pub fn repertoire_modeles_par_defaut() -> PathBuf {
    repertoire_donnees_app().join("modeles")
}

/// Construit l'état initial d'une nouvelle histoire (personnage + premier
/// combat), sans rien écrire sur disque ni toucher à l'index — pure,
/// testable isolément. `lancer()` s'occupe de la génération de l'id, de
/// l'écriture de l'index et de la sauvegarde une fois le modèle chargé.
pub fn creer_nouvelle_histoire(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
    de: &mut dyn GenerateurDe,
) -> Result<EtatPartie, ErreurCreationPersonnage> {
    let personnage = creer_personnage(nom, classe, valeurs)?;
    let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
    let combat = commencer_combat(&personnage, vec![gobelin], de);
    Ok(EtatPartie {
        personnage,
        lieu_actuel: "Clairière".to_string(),
        combat_en_cours: Some(combat),
        resumes: Vec::new(),
        pnj_connus: Vec::new(),
    })
}

/// Charge l'état d'une histoire existante depuis son fichier de
/// sauvegarde. Séparé de `lancer()` pour rester testable isolément (Review
/// Focus : fichier absent ou corrompu doit produire une erreur propre,
/// pas une panique).
fn charger_histoire_existante(chemin_sauvegarde: &Path) -> Result<EtatPartie, ErreurSauvegarde> {
    let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde.to_path_buf());
    charger_partie(&sauvegarde)
}

/// Contexte d'une histoire en cours, regroupé pour ne pas faire exploser
/// le nombre de paramètres de `traiter_message`.
pub struct ContexteHistoire<'a> {
    pub sauvegarde: &'a dyn Sauvegarde,
    pub chemin_sauvegarde: &'a Path,
    pub repertoire_sauvegardes: &'a Path,
    pub id: &'a str,
}

pub fn traiter_message(
    etat: &mut Option<EtatPartie>,
    message: MessageVersWorker,
    narrateur: &mut dyn Narrateur,
    de: &mut dyn GenerateurDe,
    contexte: &ContexteHistoire,
    sur_message_ui: &mut dyn FnMut(MessageVersUi),
) {
    match message {
        MessageVersWorker::JouerTour(saisie) => {
            let Some(etat_courant) = etat.as_mut() else { return };
            let resultat = JouerTour::executer(etat_courant, &saisie, narrateur, de, &mut |fragment: &str| {
                sur_message_ui(MessageVersUi::NarrationFragment(fragment.to_string()));
            });

            match resultat {
                Ok(resultat) => {
                    if resultat.partie_terminee {
                        let _ = std::fs::remove_file(contexte.chemin_sauvegarde);
                        let _ = histoires::retirer(contexte.repertoire_sauvegardes, contexte.id);
                    } else {
                        let _ = sauvegarder_partie(contexte.sauvegarde, etat_courant);
                    }
                    sur_message_ui(MessageVersUi::TourTermine {
                        etat: etat_courant.clone(),
                        resultat_narration: resultat.narration,
                        resume_mecanique: resultat.resume_mecanique,
                        vainqueur_combat: resultat.vainqueur_combat,
                        partie_terminee: resultat.partie_terminee,
                    });
                }
                Err(erreur) => {
                    sur_message_ui(MessageVersUi::TourEchoue(format!("{:?}", erreur)));
                }
            }
        }

        // Ces messages n'atteignent jamais `traiter_message` en pratique :
        // `ReprendreHistoire`/`CreerPersonnage` sont consommés par la
        // boucle externe de `lancer()` avant qu'une histoire soit
        // chargée ; `RetourSelection` par `jouer_histoire` ; et
        // `ConfirmerTelechargement` par `lancer()` pendant le
        // téléchargement. L'exhaustivité du `match` impose ces bras.
        MessageVersWorker::ReprendreHistoire(_)
        | MessageVersWorker::CreerPersonnage { .. }
        | MessageVersWorker::RetourSelection
        | MessageVersWorker::ConfirmerTelechargement(_) => {}
    }
}

pub enum ActionApresHistoire {
    RetourSelection,
    CanalFerme,
}

/// Traite les tours d'une histoire déjà chargée jusqu'à un retour
/// explicite à la sélection (`RetourSelection`, envoyé par le bouton
/// "Nouvelle partie" de l'écran de fin) ou la fermeture du canal.
fn jouer_histoire(
    rx: &std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: &futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    etat: &mut Option<EtatPartie>,
    narrateur: &mut dyn Narrateur,
    de: &mut dyn GenerateurDe,
    contexte: &ContexteHistoire,
) -> ActionApresHistoire {
    loop {
        match rx.recv() {
            Ok(MessageVersWorker::RetourSelection) => return ActionApresHistoire::RetourSelection,
            Ok(message) => traiter_message(etat, message, narrateur, de, contexte, &mut |msg| {
                let _ = tx.unbounded_send(msg);
            }),
            Err(_) => return ActionApresHistoire::CanalFerme,
        }
    }
}

pub fn lancer(
    rx: std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    repertoire_sauvegardes: PathBuf,
    repertoire_modeles: PathBuf,
) {
    loop {
        let histoires_existantes = histoires::lister(&repertoire_sauvegardes);
        let _ = tx.unbounded_send(MessageVersUi::ListeHistoires(histoires_existantes.clone()));

        let (id, info, etat_initial, deja_enregistree) = loop {
            match rx.recv() {
                Ok(MessageVersWorker::ReprendreHistoire(id_choisie)) => {
                    let Some(info) = histoires_existantes.iter().find(|h| h.id == id_choisie).cloned() else {
                        continue; // id inconnu : on ignore, l'UI reste sur la liste déjà envoyée
                    };
                    let chemin = histoires::chemin_sauvegarde(&repertoire_sauvegardes, &id_choisie);
                    match charger_histoire_existante(&chemin) {
                        Ok(etat) => break (id_choisie, info, etat, true),
                        Err(erreur) => {
                            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                                "échec de lecture de la sauvegarde : {erreur:?}"
                            )));
                            continue;
                        }
                    }
                }
                Ok(MessageVersWorker::CreerPersonnage { nom, classe, valeurs, palette_modele }) => {
                    let mut de_creation = infra::GenerateurDeOs;
                    let Ok(etat) = creer_nouvelle_histoire(&nom, &classe, valeurs, &mut de_creation) else {
                        continue; // formulaire invalide : ne devrait pas arriver, la Création valide déjà
                    };
                    let nouvel_id = histoires::generer_id(&nom);
                    let info = InfoHistoire {
                        id: nouvel_id.clone(),
                        nom_personnage: nom,
                        classe,
                        palette_modele,
                        cree_le: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0),
                    };
                    break (nouvel_id, info, etat, false);
                }
                Ok(_) => continue,
                Err(_) => return,
            }
        };

        let chemin_modele = info.palette_modele.chemin_dans(&repertoire_modeles);

        if crate::telechargement::faut_il_telecharger(&chemin_modele) {
            let taille_octets = match crate::telechargement::verifier_taille_modele(&info.palette_modele) {
                Ok(taille) => taille,
                Err(erreur) => {
                    let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                        "échec de la vérification du modèle à télécharger : {erreur:?}"
                    )));
                    continue;
                }
            };

            let _ = tx.unbounded_send(MessageVersUi::ConfirmationTelechargementRequise {
                palette: info.palette_modele,
                taille_octets,
            });

            let confirme = loop {
                match rx.recv() {
                    Ok(MessageVersWorker::ConfirmerTelechargement(reponse)) => break reponse,
                    Ok(_) => continue,
                    Err(_) => return,
                }
            };

            if !confirme {
                continue; // retour à la sélection — annulation volontaire, pas une erreur
            }

            let resultat = crate::telechargement::telecharger_modele(&info.palette_modele, &chemin_modele, &mut |progression| {
                let _ = tx.unbounded_send(MessageVersUi::ProgressionTelechargement(progression));
            });
            if let Err(erreur) = resultat {
                let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                    "échec du téléchargement du modèle : {erreur:?}"
                )));
                continue;
            }
        }

        let mut narrateur = match infra::LlamaNarrateur::charger(&chemin_modele) {
            Ok(narrateur) => narrateur,
            Err(erreur) => {
                let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                    "échec du chargement du modèle : {erreur}"
                )));
                continue;
            }
        };
        let mut de = infra::GenerateurDeOs;
        let chemin_sauvegarde_histoire = histoires::chemin_sauvegarde(&repertoire_sauvegardes, &id);
        let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde_histoire.clone());

        // Nouvelle histoire seulement : l'enregistrer maintenant que le
        // modèle a réellement chargé — pas avant, pour ne jamais laisser
        // une entrée fantôme dans l'index si le chargement échoue ou si
        // l'utilisateur annule le téléchargement.
        if !deja_enregistree {
            let _ = histoires::ajouter(&repertoire_sauvegardes, info.clone());
            let _ = sauvegarder_partie(&sauvegarde, &etat_initial);
        }

        let _ = tx.unbounded_send(MessageVersUi::PartieCreee(etat_initial.clone()));

        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin_sauvegarde_histoire,
            repertoire_sauvegardes: &repertoire_sauvegardes,
            id: &id,
        };
        let mut etat = Some(etat_initial);
        match jouer_histoire(&rx, &tx, &mut etat, &mut narrateur, &mut de, &contexte) {
            ActionApresHistoire::RetourSelection => continue,
            ActionApresHistoire::CanalFerme => return,
        }
    }
}
```

- [ ] **Step 6: Vérifier que le crate compile**

Run: `cargo check -p app_desktop`
Expected: FAIL encore — seul `main.rs` (Task 5) référence encore les anciennes formes de `Vue`/messages et les anciens noms de fonctions de `worker.rs`. C'est attendu à ce stade.

#### Étape D — tests de `worker.rs`

- [ ] **Step 7: Remplacer le module de tests existant**

Remplacer tout le contenu de `#[cfg(test)] mod tests { ... }` dans `worker.rs` par :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use infra::FauxNarrateur;
    use infra::SauvegardeFichier;
    use std::path::PathBuf;

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("app_desktop_test_{}_{}.json", std::process::id(), nom))
    }

    fn repertoire_temporaire(nom: &str) -> PathBuf {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_worker_test_{}_{}",
            std::process::id(),
            nom
        ));
        let _ = std::fs::remove_dir_all(&chemin);
        chemin
    }

    struct GenerateurDeFixe;

    impl GenerateurDe for GenerateurDeFixe {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            domain::JetDe::nouveau(1, modificateur)
        }
    }

    #[test]
    fn creer_nouvelle_histoire_construit_un_personnage_en_combat() {
        let mut de = GenerateurDeFixe;
        let etat = creer_nouvelle_histoire("Elowen", "Guerrier", [15, 14, 13, 12, 10, 8], &mut de).unwrap();
        assert_eq!(etat.personnage.nom, "Elowen");
        assert!(etat.combat_en_cours.is_some());
    }

    /// Review Focus : reprendre une histoire dont le fichier est absent ou
    /// corrompu doit produire une erreur propre, pas une panique.
    #[test]
    fn charger_histoire_existante_echoue_proprement_si_fichier_absent() {
        let chemin = chemin_temporaire("absent");
        let _ = std::fs::remove_file(&chemin);
        assert!(charger_histoire_existante(&chemin).is_err());
    }

    #[test]
    fn charger_histoire_existante_echoue_proprement_si_json_invalide() {
        let chemin = chemin_temporaire("corrompu");
        std::fs::write(&chemin, "ceci n'est pas du json valide").unwrap();
        assert!(charger_histoire_existante(&chemin).is_err());
        let _ = std::fs::remove_file(&chemin);
    }

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
        let repertoire = repertoire_temporaire("tour_reussi");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourReussi;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { partie_terminee: false, .. })));

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
        let repertoire = repertoire_temporaire("tour_echec");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourEnEchec;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'attaque".to_string()),
            &mut narrateur,
            &mut de,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourEchoue(_))));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurMort;

    impl Narrateur for NarrateurMort {
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

    /// Review Focus : la mort du personnage retire son histoire de
    /// l'index ET son fichier, sans toucher à une AUTRE histoire présente.
    #[test]
    fn mort_du_joueur_retire_son_histoire_sans_toucher_aux_autres() {
        let repertoire = repertoire_temporaire("mort");
        let chemin = histoires::chemin_sauvegarde(&repertoire, "elowen-test");
        std::fs::create_dir_all(&repertoire).unwrap();
        std::fs::write(&chemin, "contenu factice").unwrap();
        histoires::ajouter(&repertoire, InfoHistoire {
            id: "elowen-test".to_string(),
            nom_personnage: "Elowen".to_string(),
            classe: "Guerrier".to_string(),
            palette_modele: crate::modeles::PaletteModele::Equilibre,
            cree_le: 1,
        }).unwrap();
        histoires::ajouter(&repertoire, InfoHistoire {
            id: "thorn-autre".to_string(),
            nom_personnage: "Thorn".to_string(),
            classe: "Voleur".to_string(),
            palette_modele: crate::modeles::PaletteModele::Leger,
            cree_le: 2,
        }).unwrap();

        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat_en_combat = etat_sans_combat();
        etat_en_combat.combat_en_cours = Some(domain::Combat {
            ennemis: vec![domain::Ennemi::nouveau("Gobelin".into(), 7, domain::De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        etat_en_combat.personnage.subir_degats(etat_en_combat.personnage.pv_max);
        let mut etat: Option<EtatPartie> = Some(etat_en_combat);
        let mut narrateur = NarrateurMort;
        let mut de = GenerateurDeSeedLocal::nouveau(vec![10, 4]);
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { partie_terminee: true, .. })));
        assert!(!chemin.exists(), "le fichier de l'histoire morte doit être supprimé");
        let histoires_restantes = histoires::lister(&repertoire);
        assert_eq!(histoires_restantes.len(), 1);
        assert_eq!(histoires_restantes[0].id, "thorn-autre", "l'autre histoire doit survivre");

        let _ = std::fs::remove_dir_all(&repertoire);
    }

    struct GenerateurDeSeedLocal {
        valeurs: std::collections::VecDeque<i32>,
    }

    impl GenerateurDeSeedLocal {
        fn nouveau(valeurs: Vec<i32>) -> Self {
            Self { valeurs: valeurs.into() }
        }
    }

    impl GenerateurDe for GenerateurDeSeedLocal {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            let brut = self.valeurs.pop_front().expect("plus de valeurs de dé");
            domain::JetDe::nouveau(brut, modificateur)
        }
    }

    /// Review Focus : deux histoires différentes choisissant le même
    /// palier doivent résoudre vers le même fichier de modèle partagé.
    #[test]
    fn deux_histoires_meme_palette_partagent_le_meme_chemin_modele() {
        use crate::modeles::PaletteModele;
        let repertoire_modeles = PathBuf::from("modeles");
        let chemin1 = PaletteModele::Equilibre.chemin_dans(&repertoire_modeles);
        let chemin2 = PaletteModele::Equilibre.chemin_dans(&repertoire_modeles);
        assert_eq!(chemin1, chemin2);
    }
}
```

- [ ] **Step 8: Vérifier que les tests de `worker.rs` passent**

Run: `cargo test -p app_desktop worker`
Expected: PASS (7 tests). Les deux anciens tests `creer_personnage_envoie_partie_creee` et `nouvelle_partie_reinitialise_l_etat_et_supprime_la_sauvegarde` ont disparu (leurs scénarios n'existent plus : création et retour-à-la-sélection sont désormais gérés par la boucle externe de `lancer()`, pas par `traiter_message`) ; `creer_nouvelle_histoire_construit_un_personnage_en_combat` les remplace pour la partie testable isolément.

- [ ] **Step 9: Commit**

```bash
git add crates/app_desktop/src/protocole.rs crates/app_desktop/src/telechargement.rs crates/app_desktop/src/worker.rs
git commit -m "feat(app_desktop): rewrite the protocol and worker for multiple stories with per-story models"
```

(Le crate ne compile pas encore intégralement — `main.rs`, `creation.rs` restent à jour : Tasks 4-5.)

---

### Task 4: Vues — écran de sélection et sélecteur de modèle à la création

**Files:**
- Create: `crates/app_desktop/src/vues/selection_histoire.rs`
- Modify: `crates/app_desktop/src/vues/creation.rs`
- Modify: `crates/app_desktop/src/vues/mod.rs`
- Modify: `crates/app_desktop/src/styles.rs`

**Interfaces:**
- Consumes: `histoires::InfoHistoire` (Task 2), `modeles::PaletteModele` (Task 1), `protocole::MessageVersWorker` (Task 3).
- Produces: composant `vues::selection_histoire::SelectionHistoire { histoires: Vec<InfoHistoire>, sur_reprendre: EventHandler<MessageVersWorker>, sur_nouvelle_histoire: EventHandler<()> }` ; `Creation` gagne un champ palette et envoie `palette_modele` dans `CreerPersonnage`.

- [ ] **Step 1: Créer l'écran de sélection**

```rust
// crates/app_desktop/src/vues/selection_histoire.rs
use dioxus::prelude::*;

use crate::histoires::InfoHistoire;
use crate::protocole::MessageVersWorker;

#[component]
pub fn SelectionHistoire(
    histoires: Vec<InfoHistoire>,
    sur_reprendre: EventHandler<MessageVersWorker>,
    sur_nouvelle_histoire: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Tes histoires" }
                if histoires.is_empty() {
                    p { "Aucune histoire pour l'instant." }
                } else {
                    for histoire in histoires.iter() {
                        div {
                            key: "{histoire.id}",
                            class: "pnj",
                            onclick: {
                                let id = histoire.id.clone();
                                move |_| sur_reprendre.call(MessageVersWorker::ReprendreHistoire(id.clone()))
                            },
                            strong { "{histoire.nom_personnage}" }
                            span { " — {histoire.classe} — {histoire.palette_modele.nom_affiche()}" }
                        }
                    }
                }
                button {
                    class: "bouton-principal",
                    onclick: move |_| sur_nouvelle_histoire.call(()),
                    "Nouvelle histoire"
                }
            }
        }
    }
}
```

- [ ] **Step 2: Déclarer le module**

Dans `crates/app_desktop/src/vues/mod.rs`, ajouter `pub mod selection_histoire;` (ordre alphabétique avec les autres).

- [ ] **Step 3: Ajouter le sélecteur de modèle à `Creation`**

Dans `crates/app_desktop/src/vues/creation.rs` :

Ajouter l'import : `use crate::modeles::PaletteModele;`

Ajouter un signal après `valeurs` : `let mut palette = use_signal(|| PaletteModele::Equilibre);`

Ajouter ce bloc dans le `rsx!`, juste après le bloc `.champ` de la classe (avant la boucle sur `NOMS_CARACTERISTIQUES`) :

```rust
div { class: "champ",
    label { "Modèle : " }
    select {
        value: "{palette().nom_affiche()}",
        onchange: move |evt| {
            if let Some(p) = PaletteModele::TOUTES.iter().find(|p| p.nom_affiche() == evt.value()) {
                palette.set(*p);
            }
        },
        for p in PaletteModele::TOUTES.iter() {
            option {
                key: "{p.nom_affiche()}",
                value: "{p.nom_affiche()}",
                "{p.nom_affiche()} (~{crate::telechargement::formater_taille_go(p.taille_approx_octets())})"
            }
        }
    }
    if let Some(avertissement) = palette().avertissement() {
        p { class: "avertissement", "{avertissement}" }
    }
}
```

Dans le `onclick` du bouton `"Commencer l'aventure"`, ajouter `palette_modele: palette(),` au constructeur `MessageVersWorker::CreerPersonnage { ... }`.

- [ ] **Step 4: Ajouter un style pour le texte d'avertissement hors du journal**

`.avertissement` existe déjà pour le journal (`.journal .avertissement`, couleur `--danger`) mais n'est pas stylé en dehors. Dans `crates/app_desktop/src/styles.rs`, ajouter, après le bloc `.champ select { ... }` :

```css
.champ .avertissement {
    color: var(--danger);
    font-size: 0.8rem;
    margin-top: 0.25rem;
}
```

- [ ] **Step 5: Vérifier que les deux vues compilent isolément**

Run: `cargo check -p app_desktop 2>&1 | grep -E "selection_histoire|creation.rs"`
Expected: aucune erreur pointant vers ces deux fichiers spécifiquement (le crate entier ne compile pas encore tant que `main.rs` n'est pas à jour — Task 5 — mais ces deux fichiers eux-mêmes doivent être syntaxiquement et typographiquement corrects par rapport aux interfaces déjà posées dans Tasks 1-3).

- [ ] **Step 6: Commit**

```bash
git add crates/app_desktop/src/vues/selection_histoire.rs crates/app_desktop/src/vues/creation.rs crates/app_desktop/src/vues/mod.rs crates/app_desktop/src/styles.rs
git commit -m "feat(app_desktop): add the story-selection screen and a model picker in character creation"
```

---

### Task 5: Câblage final dans `main.rs` et mise à jour des vues de téléchargement

**Files:**
- Modify: `crates/app_desktop/src/main.rs`
- Modify: `crates/app_desktop/src/vues/telechargement.rs`
- Modify: `crates/app_desktop/src/vues/confirmation_telechargement.rs`
- Modify: `crates/app_desktop/src/vues/fin_de_partie.rs` (libellé du bouton seulement, si besoin)

**Interfaces:**
- Consumes: tout ce que les Tasks 1-4 produisent.

- [ ] **Step 1: Déclarer les nouveaux modules**

Dans `crates/app_desktop/src/main.rs`, ajouter `mod histoires;` et `mod modeles;` à côté des autres `mod` (ordre alphabétique : `histoires`, `modeles`, `protocole`, `styles`, `telechargement`, `vues`, `worker`).

Ajouter l'import `use vues::selection_histoire::SelectionHistoire;` à côté des autres imports de vues.

- [ ] **Step 2: Mettre à jour `fn main()`**

Remplacer :

```rust
    let chemin_modele = worker::chemin_modele_par_defaut();
    let chemin_sauvegarde = worker::chemin_sauvegarde_par_defaut();
```

par :

```rust
    let repertoire_sauvegardes = worker::repertoire_sauvegardes_par_defaut();
    let repertoire_modeles = worker::repertoire_modeles_par_defaut();
```

et dans la fermeture passée à `std::thread::spawn`, remplacer les noms de variables clonées/l'appel à `worker::lancer` en conséquence (mêmes clonages, nouveaux noms, nouvel ordre d'arguments correspondant à la nouvelle signature `lancer(rx, tx, repertoire_sauvegardes, repertoire_modeles)`).

- [ ] **Step 3: Mettre à jour l'état initial de `App`**

Remplacer `let mut vue = use_signal(|| Vue::Creation);` par `let mut vue = use_signal(|| Vue::SelectionHistoire(Vec::new()));` (écran vide en attendant le premier `ListeHistoires`, envoyé immédiatement par `lancer()`).

Supprimer le signal `erreur_chargement_initiale` (devenu inutile, voir Step 4).

- [ ] **Step 4: Réécrire le `match message` du `use_future`**

Remplacer tout le bloc `match message { ... }` par :

```rust
match message {
    MessageVersUi::ListeHistoires(histoires) => {
        vue.set(Vue::SelectionHistoire(histoires));
    }
    MessageVersUi::PartieCreee(e) => {
        etat.set(Some(e));
        vue.set(Vue::Jeu);
    }
    MessageVersUi::NarrationFragment(fragment) => {
        narration_en_cours.write().push_str(&fragment);
        faire_defiler_journal();
    }
    MessageVersUi::TourTermine { etat: nouvel_etat, resultat_narration, resume_mecanique, vainqueur_combat, partie_terminee } => {
        journal.write().push(resultat_narration);
        for ligne in resume_mecanique {
            journal.write().push(ligne);
        }
        if !partie_terminee {
            match vainqueur_combat {
                Some(Vainqueur::Joueur) => journal.write().push("Victoire ! Le gobelin est vaincu.".to_string()),
                Some(Vainqueur::Fuite) => journal.write().push("Tu as fui le combat.".to_string()),
                Some(Vainqueur::Ennemis) | None => {}
            }
        }
        narration_en_cours.set(String::new());
        tour_en_cours.set(false);
        faire_defiler_journal();
        etat.set(Some(nouvel_etat));
        if partie_terminee {
            vue.set(Vue::FinDePartie(vainqueur_combat));
        }
    }
    MessageVersUi::TourEchoue(message) => {
        // Avant Vue::Jeu (sélection, création, téléchargement, chargement
        // du modèle), un échec est récupérable : `lancer()` revient à la
        // sélection juste après (nouveau ListeHistoires à suivre) — on
        // affiche l'erreur brièvement. Pendant Vue::Jeu, un échec de tour
        // reste local au journal, comme avant.
        if matches!(vue(), Vue::Jeu) {
            journal.write().push(format!("⚠ {}", message));
            narration_en_cours.set(String::new());
            tour_en_cours.set(false);
            faire_defiler_journal();
        } else {
            vue.set(Vue::ErreurChargement(message));
        }
    }
    MessageVersUi::ProgressionTelechargement(progression) => {
        if let Vue::Telechargement { palette, .. } | Vue::ConfirmationTelechargement { palette, .. } = vue() {
            vue.set(Vue::Telechargement { palette, progression });
        }
    }
    MessageVersUi::ConfirmationTelechargementRequise { palette, taille_octets } => {
        vue.set(Vue::ConfirmationTelechargement { palette, taille_octets });
    }
}
```

(`ProgressionTelechargement` retrouve la palette depuis la vue courante — déjà posée par `ConfirmationTelechargementRequise` juste avant — plutôt que de faire porter la palette par ce message aussi.)

- [ ] **Step 5: Réécrire le `match vue()` qui construit `vue_active`**

Remplacer tout le bloc par :

```rust
let vue_active = match vue() {
    Vue::ErreurChargement(message) => rsx! { ErreurChargement { message } },
    Vue::SelectionHistoire(histoires) => rsx! {
        SelectionHistoire {
            histoires,
            sur_reprendre: move |msg| { let _ = tx.send(msg); },
            sur_nouvelle_histoire: move |_| vue.set(Vue::Creation),
        }
    },
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
                    narration_en_cours.set(String::new());
                    let _ = tx.send(msg);
                },
            }
        }
    }
    Vue::Telechargement { palette, progression } => rsx! { Telechargement { palette, progression } },
    Vue::ConfirmationTelechargement { palette, taille_octets } => rsx! {
        ConfirmationTelechargement {
            palette,
            taille_octets,
            sur_reponse: move |msg| { let _ = tx.send(msg); }
        }
    },
    Vue::FinDePartie(vainqueur) => rsx! {
        FinDePartie {
            vainqueur,
            sur_nouvelle_partie: move |_| { let _ = tx.send(MessageVersWorker::RetourSelection); }
        }
    },
};
```

Note : `Vue::SelectionHistoire => ... sur_nouvelle_histoire: move |_| vue.set(Vue::Creation)` change la vue **localement**, côté UI seulement — aucun message n'est envoyé au worker à cet instant (il attend toujours sur sa boucle de sélection). C'est seulement à la soumission du formulaire de `Creation` que `MessageVersWorker::CreerPersonnage` part réellement.

- [ ] **Step 6: Mettre à jour `vues/telechargement.rs` pour afficher la palette**

Ajouter un paramètre `palette: PaletteModele` au composant `Telechargement` (import `use crate::modeles::PaletteModele;`) et l'inclure dans le texte affiché, par exemple en remplaçant `"Premier lancement : téléchargement du modèle de narration ({pourcentage}%)."` par `"Téléchargement du modèle {palette.nom_affiche()} ({pourcentage}%)."`.

- [ ] **Step 7: Mettre à jour `vues/confirmation_telechargement.rs` pour afficher la palette**

Ajouter un paramètre `palette: PaletteModele` au composant et l'inclure dans le texte, par exemple remplacer `"Télécharger le modèle de narration ({taille}) ?"` par `"Télécharger le modèle {palette.nom_affiche()} ({taille}) ?"`.

- [ ] **Step 8: Vérifier que le crate compile entièrement**

Run: `cargo build -p app_desktop`
Expected: PASS, aucune erreur.

- [ ] **Step 9: Vérifier la suite de tests complète**

Run: `cargo test --workspace`
Expected: PASS, 0 échec.

- [ ] **Step 10: Vérification manuelle sur desktop**

Avec le modèle déjà présent localement (`modeles/Qwen2.5-3B-Instruct-Q4_K_M.gguf`, déjà sur disque — coïncide exactement avec le nom de fichier attendu pour la palette Équilibré, donc aucun téléchargement ne doit se déclencher) :

```bash
rm -rf sauvegardes
cargo run -p app_desktop
```

Vérifier visuellement (capture d'écran si possible, même technique que les vérifications précédentes de ce projet) :
- L'écran de sélection s'affiche au lancement, vide, avec le bouton "Nouvelle histoire".
- Créer un personnage avec la palette Équilibré (déjà présente) : passe directement en jeu sans écran de téléchargement.
- Gagner ou fuir le combat du gobelin : le jeu continue (pas d'écran de fin), un message "Victoire !"/"Tu as fui" apparaît dans le journal.
- Tuer le personnage (ou redémarrer l'app) pour revenir à la sélection : l'histoire créée apparaît dans la liste.
- **Review Focus — annulation de téléchargement** : créer une deuxième histoire avec la palette Léger (absente du disque, déclenche l'écran de confirmation) et cliquer "Annuler" : retour à la sélection, et l'histoire annulée **n'apparaît pas** dans la liste (pas d'entrée fantôme).
- **Point flagué par le spec — rechargement séquentiel du modèle** : depuis la sélection, créer une deuxième histoire (même palette Équilibré suffit, pas besoin d'un nouveau téléchargement) : le modèle doit se recharger correctement pour cette nouvelle histoire sans que l'app plante — c'est la vérification empirique demandée par le spec pour le chargement séquentiel de `LlamaNarrateur` dans le même processus.

- [ ] **Step 11: Commit**

```bash
git add crates/app_desktop/src/main.rs crates/app_desktop/src/vues/telechargement.rs crates/app_desktop/src/vues/confirmation_telechargement.rs
git commit -m "feat(app_desktop): wire multiple stories and per-story models into the app"
```
