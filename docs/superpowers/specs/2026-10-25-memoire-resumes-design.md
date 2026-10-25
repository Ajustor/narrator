# Design — Résumés automatiques de mémoire

Date : 2026-10-25
Auteur : Alexandre Gambier (avec Claude)
Contexte : troisième brique de l'étape 2 de la feuille de route ("Prompts et mémoire"). Le port `Narrateur` et l'intégration LLM réelle (`LlamaNarrateur`) existent déjà et sont en production (voir `docs/superpowers/specs/2026-10-24-llm-narrateur-design.md`). Ce sous-projet traite l'autre moitié de l'étape 2, explicitement mise hors scope à l'époque : les résumés automatiques de mémoire. Les fiches PNJ (personnages non-joueurs récurrents) restent un sous-projet séparé, plus tard — le jeu n'a aujourd'hui aucune notion de PNJ en dehors des ennemis de combat.

## Objectif

Donner au maître du jeu une mémoire qui dépasse la fenêtre de contexte du modèle : au fil d'une longue partie, la narration doit rester cohérente avec ce qui s'est passé des dizaines de tours plus tôt, sans que le prompt envoyé au modèle ne grossisse indéfiniment. Le champ `domain::EtatPartie::resumes` existe déjà (scaffolding de l'étape 1) mais n'est aujourd'hui jamais rempli ni utilisé — ce sous-projet le rend vivant.

## Hors scope

- Fiches PNJ / suivi de personnages récurrents (sous-projet séparé, étape 2 suite).
- Tout changement au flux de classification (`proposer_action`) — les résumés n'y sont pas injectés (voir Décision ci-dessous).
- Tout changement à la contrainte de génération (grammaire GBNF désactivée depuis `4d7f9e4` à cause d'un bug non résolu de llama.cpp) — hors sujet ici, non réactivé par ce sous-projet.

## Décision d'architecture clé : une troisième méthode sur le port `Narrateur`

Le port actuel a deux méthodes (`proposer_action`, `raconter_resultat`). Il en gagne une troisième, dédiée à la compression :

```rust
pub trait Narrateur: Send {
    fn proposer_action(&mut self, contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur>;
    fn raconter_resultat(&mut self, contexte: &ContexteResultat, sur_fragment: &mut dyn FnMut(&str)) -> Result<String, ErreurNarrateur>;
    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur>;
}
```

`condenser_resumes` prend une tranche de résumés de tours (du plus ancien au plus récent) et renvoie **une seule** chaîne condensée qui les remplace tous. La décision de *quels* résumés sont « anciens » (à condenser) par opposition à « récents » (gardés tels quels) reste dans `application` — le LLM ne fait que compresser du texte qu'on lui donne, il ne décide pas de la coupure. Ce découpage garde la logique de seuil testable sans modèle réel, dans le même esprit que le découpage `proposer_action`/`raconter_resultat` de l'étape précédente.

Pas de génération contrainte pour cet appel (texte libre, comme `raconter_resultat`) — pas de streaming non plus : le résultat n'est jamais montré au joueur.

## Constantes (`application`)

```rust
pub const SEUIL_MOTS_RESUMES: usize = 800;
pub const NB_RESUMES_RECENTS_CONSERVES: usize = 5;
```

Le compte de mots (pas de tokens — `application` n'a pas accès au tokenizer du modèle, qui vit dans `infra`) est une approximation volontairement grossière ; le but est d'éviter une croissance illimitée, pas de viser une taille de contexte exacte.

## Flux révisé dans `JouerTour::executer`

Après l'étape existante 4 (`raconter_resultat` produit la narration), deux étapes s'ajoutent :

5. `etat.resumes.push(narration.clone())` — la narration du tour devient directement son propre résumé. Pas d'appel LLM dédié : la narration est déjà courte (2-3 phrases, contrainte par le prompt existant) donc déjà un bon résumé.
6. Si `etat.resumes.iter().map(|r| r.split_whitespace().count()).sum::<usize>() > SEUIL_MOTS_RESUMES` et que `etat.resumes.len() > NB_RESUMES_RECENTS_CONSERVES` :
   - couper `etat.resumes` en `anciens` (tout sauf les `NB_RESUMES_RECENTS_CONSERVES` derniers) et `recents` (les `NB_RESUMES_RECENTS_CONSERVES` derniers) ;
   - appeler `narrateur.condenser_resumes(&anciens)` ;
   - si `Ok(condense)` : remplacer `etat.resumes` par `[condense, ...recents]` ;
   - si `Err(_)` : ne rien changer — `etat.resumes` reste tel quel (au-dessus du seuil), la condensation sera retentée au tour suivant.

Ce comportement est invisible pour le joueur dans les deux cas (succès ou échec de la condensation) : pas de message, pas de log visible côté `app`. L'`EtatPartie` mis à jour (résumés inclus) est persisté via `Sauvegarde` exactement comme avant — aucun changement au port `Sauvegarde` ni au format de sauvegarde (le champ existe déjà dans le JSON sérialisé).

## Prompt — narration (`construire_prompt_narration`)

Nouvelle section ajoutée avant le résultat mécanique du tour, construite à partir de `contexte.etat.resumes` :

```
Ce qui s'est passé jusqu'ici : {résumés joints par un espace}
```

Absente (chaîne vide, comme pour `description_combat` hors combat) quand `resumes` est vide — ce qui n'arrive en pratique qu'à la création d'une partie, avant le premier tour.

## Prompt — classification (`construire_prompt_classification`)

**Inchangé.** La classification ne reçoit que l'état courant (lieu, combat, PV, inventaire) — l'historique n'aide pas à choisir entre les 4 formats d'action et alourdirait un prompt volontairement court.

## Prompt — condensation (`construire_prompt_condensation`, nouveau)

Prompt dédié, séparé des deux autres :

```
Voici plusieurs résumés successifs d'une histoire de jeu de rôle en français :
{résumés joints, un par ligne}
Condense-les en un seul paragraphe court (3-4 phrases maximum) en français,
qui garde les faits importants (lieux visités, événements marquants, objets
obtenus, ennemis vaincus) sans les détails mineurs. Réponds uniquement par
ce paragraphe, sans aucun autre texte.
```

## Fichiers (`application`)

- `crates/application/src/ports/narrateur.rs` : ajout de `condenser_resumes` au trait `Narrateur`.
- `crates/application/src/usecases/jouer_tour.rs` : ajout des étapes 5-6 ci-dessus, ajout des constantes `SEUIL_MOTS_RESUMES` / `NB_RESUMES_RECENTS_CONSERVES`.

## Fichiers (`infra`)

- `crates/infra/src/llama_narrateur.rs` : `LlamaNarrateur::condenser_resumes` (même mécanique que `raconter_resultat` côté génération : chat template, pas de grammaire, pas de streaming).
- `crates/infra/src/prompts.rs` : `construire_prompt_condensation` (nouveau), `construire_prompt_narration` modifié pour inclure `contexte.etat.resumes`.
- `crates/infra/src/faux_narrateur.rs` : `FauxNarrateur` gagne une implémentation de `condenser_resumes`, configurable pour retourner `Ok` ou `Err` selon le scénario de test (double de test, pas de changement de comportement par défaut pour les tests existants).

## Stratégie de test

- Pur et testable sans modèle : `construire_prompt_condensation` (contient les résumés d'entrée, demande le français et la brièveté), `construire_prompt_narration` modifié (contient les résumés de `etat.resumes`).
- `JouerTour::executer` avec `FauxNarrateur` :
  - un tour ajoute exactement la narration retournée à `etat.resumes` ;
  - un scénario où le total dépasse `SEUIL_MOTS_RESUMES` déclenche `condenser_resumes` avec la bonne tranche (`anciens` = tout sauf les `NB_RESUMES_RECENTS_CONSERVES` derniers) et le résultat remplace cette tranche ;
  - un scénario où `condenser_resumes` retourne `Err` laisse `etat.resumes` inchangé (toujours au-dessus du seuil) et le tour réussit malgré tout (narration bien renvoyée au joueur).
- `LlamaNarrateur::condenser_resumes` : pas de test automatisé (vrai modèle, non déterministe) — vérification manuelle en jouant une longue partie avec `NARRATEUR_MODELE` pointant vers un `.gguf` téléchargé, en observant (via instrumentation temporaire, retirée avant commit) qu'une condensation se déclenche et que la partie reste jouable ensuite.

## Critères de complétion

- `cargo test` passe sur tout le workspace.
- Avec `NARRATEUR_MODELE` pointant vers un modèle réel, une partie manuelle de nombreux tours montre : les résumés s'accumulent tour après tour, une condensation se déclenche une fois le seuil dépassé (observable en inspectant le fichier de sauvegarde JSON entre deux tours), et la narration reste cohérente avec les événements condensés plus tôt dans la partie.
