# Design — Fiches PNJ

Date : 2026-10-25
Auteur : Alexandre Gambier (avec Claude)
Contexte : dernière brique de l'étape 2 de la feuille de route ("Prompts et mémoire"). Les résumés automatiques (docs/superpowers/specs/2026-10-25-memoire-resumes-design.md) sont déjà en production. Ce sous-projet traite l'autre moitié, explicitement différée à l'époque : les fiches PNJ (personnages non-joueurs récurrents).

## Objectif

Permettre au maître du jeu de rester cohérent avec les personnages non-joueurs qu'il invente au fil de la narration libre : si le joueur rencontre « Aldric, forgeron bourru et méfiant » puis le recroise plus tard, le MJ ne doit pas réinventer sa personnalité à chaque fois.

## Hors scope

- Toute mécanique de jeu liée aux PNJ (relations chiffrées, quêtes, commerce) — purement narratif, comme les résumés.
- Toute UI de consultation des fiches (réservé à l'étape Dioxus).
- Les ennemis de combat (`domain::Ennemi`) ne sont pas concernés — ce sont déjà des entités structurées, sans lien avec ce sous-projet.
- Correspondance floue (« le forgeron » → « Aldric ») côté injection : seule une correspondance exacte de nom (insensible à la casse) est reconnue. Une correspondance plus robuste resterait possible via un appel LLM dédié, mais a été explicitement écartée au profit d'un filtrage déterministe, sans coût d'inférence supplémentaire.

## Décision d'architecture clé : deux mécanismes séparés, pas un seul

Un piège identifié pendant la conception : si la détection d'un PNJ se fait uniquement *après* la narration (en inspectant ce qui vient d'être raconté, comme pour `condenser_resumes`), alors la fiche d'un PNJ déjà connu ne peut jamais informer la narration du tour où il réapparaît — exactement le problème que ce sous-projet est censé résoudre. La conception sépare donc :

1. **Injection (avant la narration), déterministe, sans appel LLM** : si le nom d'un PNJ déjà connu apparaît tel quel (insensible à la casse) dans la saisie libre du joueur, sa fiche (nom + description seulement, jamais l'historique complet) est ajoutée au prompt de narration. Même esprit que le filtrage déjà utilisé ailleurs dans le code (`FauxNarrateur`, `action_schema.rs`).
2. **Détection et création (après la narration), un appel LLM, hors combat seulement** : une fois la narration produite, un appel dédié demande au modèle si un PNJ (nouveau ou déjà connu) est apparu dans ce qui vient d'être raconté. Si oui, sa fiche est créée ou mise à jour (narration du tour ajoutée à son historique). Ne tourne jamais pendant un combat actif — un combat ne porte pas l'attention sur des PNJ narratifs, et cela évite un appel LLM inutile pendant les échanges de coups (même budget de performance que la décision prise pour `condenser_resumes`).

L'historique d'un PNJ n'est **jamais** réinjecté dans un prompt — seulement nom + description. Ceci évite délibérément de reproduire le bug de croissance non bornée des tokens découvert et corrigé sur `resumes` (le batch `llama-cpp-2` a une taille liée à la longueur du prompt ; un historique qui grossit à chaque rencontre aurait posé exactement le même risque).

## Modèle de données (`domain`)

```rust
pub struct FichePnj {
    pub nom: String,
    pub description: String,
    pub historique: Vec<String>,
}
```

`EtatPartie` gagne un nouveau champ :

```rust
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
    pub pnj_connus: Vec<FichePnj>,
}
```

Persisté via `Sauvegarde` sans aucun changement de port — le format JSON sérialise le nouveau champ automatiquement (`serde`), exactement comme `resumes` à l'étape précédente.

## Décision d'architecture : une quatrième méthode sur le port `Narrateur`

```rust
pub struct PnjDetecte {
    pub nom: String,
    pub nouvelle_description: Option<String>, // Some(...) seulement si c'est un nouveau PNJ
}

pub trait Narrateur: Send {
    // ... proposer_action, raconter_resultat, condenser_resumes (inchangés)
    fn detecter_pnj(
        &mut self,
        narration: &str,
        noms_connus: &[String],
    ) -> Result<Option<PnjDetecte>, ErreurNarrateur>;
}
```

`noms_connus` permet au modèle de faire correspondre un PNJ récurrent à une fiche existante plutôt que d'inventer un doublon. Pas de génération contrainte (texte libre, comme `condenser_resumes`), pas de streaming — jamais montré au joueur.

## Flux révisé dans `JouerTour::executer`

Après les étapes existantes (narration produite, `resumes` mis à jour, condensation éventuelle), si **aucun combat actif** — réutiliser la fonction privée `combat_termine(etat: &EtatPartie) -> bool` déjà définie dans `jouer_tour.rs` (`etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(true)` : vrai s'il n'y a pas de combat, ou s'il y en a un mais qu'il est terminé) plutôt que de réécrire une condition équivalente :

1. `noms_connus = etat.pnj_connus.iter().map(|p| p.nom.clone()).collect()`
2. `narrateur.detecter_pnj(&narration, &noms_connus)`
3. Sur `Ok(Some(detecte))` :
   - Si une fiche existante correspond à `detecte.nom` (comparaison insensible à la casse) : ajouter `narration.clone()` à son `historique`.
   - Sinon : créer une nouvelle `FichePnj { nom: detecte.nom, description: detecte.nouvelle_description.unwrap_or_default(), historique: vec![narration.clone()] }` et l'ajouter à `etat.pnj_connus`.
4. Sur `Ok(None)` ou `Err(_)` : ne rien faire. Comme pour la condensation, un échec de détection n'échoue jamais le tour et reste invisible au joueur — pas de message, pas de log.

Ce comportement s'ajoute à la fin de `JouerTour::executer`, après le bloc existant de gestion de `resumes`.

## Prompt — injection (modification de `construire_prompt_narration`)

Nouvelle section, construite **avant** l'appel au modèle, à partir de `contexte.etat.pnj_connus` filtrés contre la saisie du joueur — uniquement quand `contexte.action` est `ActionJoueur::ActionLibre(texte)` (seule variante porteuse de texte libre) :

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

Insérée dans `construire_prompt_narration` au même titre que `description_memoire`/`description_combat`.

## Prompt — détection (`construire_prompt_detection_pnj`, nouveau)

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

Trois formats JSON exacts, avec exemples explicites dans le prompt — même technique que celle qui a rendu `construire_prompt_classification` fiable sans grammaire contrainte (voir `4d7f9e4`/`50fcc64`).

## Parsing (`detecter_pnj`, nouveau fichier `infra/src/detection_pnj.rs`)

```rust
pub fn interpreter_json_detection_pnj(json_brut: &str) -> Result<Option<PnjDetecte>, ErreurInterpretation>
```

Fichier séparé de `prompts.rs` et de `action_schema.rs` — responsabilité unique (parsing JSON de la détection PNJ), même esprit que la séparation déjà en place entre construction de prompt et interprétation de réponse pour la classification d'action.

## Fichiers (`domain`)

- `crates/domain/src/fiche_pnj.rs` (nouveau) : `FichePnj`.
- `crates/domain/src/etat_partie.rs` : ajout du champ `pnj_connus: Vec<FichePnj>`.
- `crates/domain/src/lib.rs` : export de `FichePnj`.

## Fichiers (`application`)

- `crates/application/src/ports/narrateur.rs` : ajout de `PnjDetecte` et de `detecter_pnj` au trait `Narrateur`.
- `crates/application/src/usecases/jouer_tour.rs` : ajout de la logique de détection/création décrite ci-dessus.

## Fichiers (`infra`)

- `crates/infra/src/prompts.rs` : `description_pnj_pertinents` (nouveau, privé), modification de `construire_prompt_narration`, `construire_prompt_detection_pnj` (nouveau, public).
- `crates/infra/src/detection_pnj.rs` (nouveau) : `interpreter_json_detection_pnj`.
- `crates/infra/src/faux_narrateur.rs` : implémentation de `detecter_pnj` (double de test).
- `crates/infra/src/llama_narrateur.rs` : implémentation de `detecter_pnj` (vrai modèle, même mécanique que `condenser_resumes`).

## Fichiers (`app`)

- `crates/app/src/main.rs` : la construction de `EtatPartie` pour une nouvelle partie initialise `pnj_connus: Vec::new()`.

## Sites de construction d'`EtatPartie` à mettre à jour

L'ajout du champ `pnj_connus` casse la compilation de tout littéral `EtatPartie { ... }` existant tant qu'il n'est pas mis à jour. Sites recensés (hors définition du type lui-même) :

- `crates/app/src/main.rs:34`
- `crates/domain/src/etat_partie.rs:19` (test)
- `crates/application/src/ports/sauvegarde.rs:40` (test)
- `crates/application/src/usecases/partie.rs:34` (test)
- `crates/application/src/usecases/jouer_tour.rs:277` (test)
- `crates/infra/src/faux_narrateur.rs:80` (test)
- `crates/infra/src/action_schema.rs:130` (test)
- `crates/infra/src/prompts.rs:133` (test)
- `crates/infra/src/llama_narrateur.rs:230` (test)
- `crates/infra/src/sauvegarde_fichier.rs:41` (test)

## Stratégie de test

- Pur et testable sans modèle : `FichePnj` (round-trip JSON, comme `EtatPartie`/`resumes`), `description_pnj_pertinents` (filtrage par correspondance de nom), `construire_prompt_detection_pnj`, `interpreter_json_detection_pnj` (les trois formats JSON + cas d'erreur).
- `JouerTour::executer` avec des doubles de test locaux (même pattern que `NarrateurCondensateur`) :
  - un PNJ détecté comme nouveau crée une fiche avec la bonne description et un historique d'une entrée ;
  - un PNJ détecté comme déjà connu ajoute la narration à l'historique de la fiche existante, sans en créer une seconde ;
  - aucune détection déclenchée pendant un combat actif, même si le double renverrait normalement un PNJ ;
  - un échec de détection (`Err`) laisse `pnj_connus` inchangé et ne fait pas échouer le tour.
- `FauxNarrateur::detecter_pnj` : un test simple, comme `FauxNarrateur::condenser_resumes`.
- `LlamaNarrateur::detecter_pnj` : pas de test automatisé (vrai modèle, non déterministe) — vérification manuelle avec un modèle réel téléchargé, en jouant plusieurs tours hors combat mentionnant un PNJ inventé, puis en revérifiant que la fiche est bien réutilisée sans être dupliquée à la rencontre suivante (inspection de `partie.json`).

## Critères de complétion

- `cargo test` passe sur tout le workspace.
- Avec `NARRATEUR_MODELE` pointant vers un modèle réel : une partie manuelle montre qu'un PNJ inventé par la narration obtient une fiche (nom + description cohérents), qu'une rencontre suivante avec ce PNJ (le joueur mentionne son nom) réutilise sa fiche dans la narration sans la dupliquer, et qu'aucun appel de détection ne se produit pendant un combat actif.
