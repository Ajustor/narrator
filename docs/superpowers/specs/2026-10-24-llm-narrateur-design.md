# Design — Intégration LLM réelle derrière le port Narrateur

Date : 2026-10-24
Auteur : Alexandre Gambier (avec Claude)
Contexte : deuxième brique de l'étape 2 de la feuille de route ("Prompts et mémoire"). Remplace le besoin de `FauxNarrateur` comme seule implémentation du port `Narrateur` par une vraie intégration LLM locale, tout en gardant `FauxNarrateur` comme double de test. Les résumés automatiques de mémoire (l'autre moitié de l'étape 2) sont un sous-projet séparé, traité plus tard.

## Objectif

Donner au moteur de jeu un vrai maître du jeu : un modèle GGUF tournant localement via `llama-cpp-2`, qui interprète la saisie libre du joueur en une action structurée, puis raconte le résultat réel (succès/échec, dégâts) une fois les dés résolus par le code — jamais l'inverse.

## Hors scope

- Résumés automatiques de mémoire / fiches PNJ (sous-projet séparé, étape 2 suite).
- Téléchargement automatique du modèle au premier lancement (fonctionnalité d'UX réservée à l'étape mobile/Android).
- Affinage des threads pour cœurs performants/efficaces (Android, étape 4).
- Interface Dioxus (étape 3).

## Décision d'architecture clé : scinder le port `Narrateur` en deux appels

Le port actuel (`application::ports::narrateur::Narrateur`, issu de l'étape 1) n'a qu'une méthode `raconter` qui renvoie narration + action en un seul appel. Pour que la narration reflète le résultat mécanique réel (et non une tentative ambiguë), les dés doivent être résolus **entre** la classification de l'action et la narration. Le port devient :

```rust
pub trait Narrateur: Send {
    fn proposer_action(
        &mut self,
        contexte: &ContexteTour,
    ) -> Result<ActionJoueur, ErreurNarrateur>;

    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur>;
}
```

`ContexteTour` reste comme avant (`{ etat: EtatPartie, saisie_joueur: String }`). `ContexteResultat` réutilise ce qui existe déjà plutôt que d'inventer un type mécanique parallèle :

```rust
pub struct ContexteResultat {
    pub etat: EtatPartie,
    pub action: ActionJoueur,
    pub resume_mecanique: Vec<String>,
}
```

`resume_mecanique` est exactement ce que `resoudre_action_joueur` produit déjà (ex: "🎲 Tu attaques Gobelin : 14 vs défense 12 — touché ! Dégâts : 5") — aucune nouvelle structure de résolution à inventer, juste un nouveau consommateur de données existantes.

**Ceci modifie du code déjà livré à l'étape 1** (le port, Tâche 9 ; l'orchestration dans `JouerTour::executer`, Tâches 13-15 ; `FauxNarrateur`, Tâche 17) — ce n'est pas un ajout pur. L'interface externe de `JouerTour::executer` (ce que `app` appelle) ne change pas : la scission est interne à l'orchestration.

## Flux révisé dans `JouerTour::executer`

1. `narrateur.proposer_action(&contexte_tour)` → `ActionJoueur` (LLM, génération contrainte par grammaire — pas de streaming, jamais montré au joueur)
2. `valider_action(&action, etat)` (inchangé)
3. Résolution mécanique complète du tour — joueur et ennemi, dans l'ordre que l'initiative dicte déjà (`ennemi_agit_avant_joueur`), logique de branchement existante inchangée (toujours déterministe en code, dés via `GenerateurDe`, jamais dans le LLM)
4. `narrateur.raconter_resultat(&contexte_resultat, sur_fragment)` → narration streamée au joueur, informée par l'ensemble du résultat mécanique du tour (`resume_mecanique` complet, joueur + ennemi)

Narration appelée une seule fois, à la fin, après que tous les dés du tour (joueur et ennemi) ont été résolus — pas seulement ceux du joueur — pour éviter le cas particulier où le joueur meurt avant d'avoir pu agir (ennemi plus rapide) et où il n'y aurait sinon rien de cohérent à raconter sur une action jamais résolue.

## Appel 1 — classification de l'action (`proposer_action`)

Prompt court : règles + état courant résumé (lieu, PV du joueur, ennemis vivants avec leurs noms, objets consommables en inventaire) + saisie du joueur. Génération contrainte par une grammaire GBNF dérivée dynamiquement d'un JSON Schema (via `llama_cpp_2::json_schema_to_grammar`) — reconstruite à chaque appel, pas de grammaire statique en dur :

```json
{"action": "attaquer", "cible": "Gobelin"}
{"action": "utiliser_objet", "objet": "Potion de soin"}
{"action": "fuir"}
{"action": "libre", "texte": "..."}
```

`cible`/`objet` sont des `enum` JSON Schema générés à partir de l'état du tour (noms des ennemis vivants, noms des objets `Consommable` en inventaire) — le modèle ne peut pas halluciner une cible qui n'existe pas. `action` est un `enum` de base à 4 valeurs, mais les valeurs non pertinentes pour ce tour sont retirées de l'`enum` `action` lui-même avant de générer la grammaire : `attaquer` si aucun ennemi n'est vivant, `utiliser_objet` si aucun objet consommable n'est en inventaire, `fuir` si aucun combat n'est en cours (même règle que `valider_action`). `libre` reste toujours disponible. Le JSON est garanti syntaxiquement valide par la grammaire ; mappé directement vers `domain::ActionJoueur` après désérialisation. Peu de tokens générés (juste l'objet JSON), donc rapide — pas de streaming, le résultat n'est jamais montré au joueur.

## Appel 2 — narration du résultat (`raconter_resultat`)

Prompt informé par `ContexteResultat` : l'action tentée et son résultat mécanique réel (`resume_mecanique`). Génération libre, streamée token par token vers `sur_fragment`. Le modèle décrit ce qui s'est **réellement** passé (touché/raté, dégâts, PV restants) — jamais l'inverse : le moteur a déjà tout calculé avant cet appel.

## Chargement du modèle & configuration

Un seul `LlamaModel` + `LlamaBackend` chargés une fois à la création de `LlamaNarrateur` (pas par tour), `mmap` activé par défaut dans llama.cpp. Contexte à 4096 tokens. Threads : `std::thread::available_parallelism()` (pas de distinction cœurs perf/efficacité — affinage réservé à l'étape Android). Chemin du `.gguf` via la variable d'environnement `NARRATEUR_MODELE` ; échec de chargement = message clair + sortie du programme (pas de repli silencieux vers `FauxNarrateur`).

**Modèle recommandé pour les tests** : Qwen2.5-3B-Instruct, quantification Q4_K_M (~2 Go), GGUF via les builds communautaires (ex: bartowski). Mature, bon en français, bon suivi d'instructions.

## Crate et dépendances

`llama-cpp-2` (v0.1.160 au 2026-10-15, maintenu activement par `utilityai`, vérifié sur crates.io) ajouté comme dépendance d'`infra` uniquement — `domain` et `application` restent indépendants du moteur d'inférence (seul le port `Narrateur` les concerne, son implémentation concrète vit dans `infra`).

## Fichiers (`infra`)

- `crates/infra/src/llama_narrateur.rs` : `LlamaNarrateur`, implémente `Narrateur` via `proposer_action`/`raconter_resultat`. Contient aussi la construction du JSON Schema dynamique et du prompt.
- `crates/infra/src/faux_narrateur.rs` : mis à jour pour implémenter les deux nouvelles méthodes du port (reste un double de test, gardé dans le code).

## Fichiers (`application`)

- `crates/application/src/ports/narrateur.rs` : port modifié (deux méthodes), `ContexteResultat` ajouté.
- `crates/application/src/usecases/jouer_tour.rs` : `JouerTour::executer` réorganisé selon le flux en 5 étapes ci-dessus.

## Fichiers (`app`)

- `crates/app/src/main.rs` : lit `NARRATEUR_MODELE`, instancie `LlamaNarrateur` (erreur claire et sortie si absent/échec de chargement) au lieu de `FauxNarrateur`.

## Stratégie de test

- Pur et testable sans modèle : construction du JSON Schema dynamique (énumérations à partir de l'état), construction des deux prompts, mapping JSON→`ActionJoueur` — TDD classique, aucune dépendance au modèle réel.
- `JouerTour::executer` (orchestration en 5 étapes) : testable avec un double `Narrateur` à deux méthodes + `GenerateurDeSeed`, comme les tests existants des Tâches 13-15 (mis à jour pour le nouveau découpage).
- `LlamaNarrateur::proposer_action`/`raconter_resultat` : pas de test automatisé (vrai modèle, non déterministe) — vérification manuelle en jouant une partie avec `NARRATEUR_MODELE` pointant vers un `.gguf` téléchargé.

## Critères de complétion

- `cargo test` passe sur tout le workspace (avec `FauxNarrateur` mis à jour, pas besoin du modèle réel pour la suite automatisée).
- Avec `NARRATEUR_MODELE` pointant vers Qwen2.5-3B-Instruct-Q4_K_M, une partie manuelle montre : une saisie libre correctement interprétée en action (attaque/objet/fuite/libre), une narration cohérente avec le résultat réel des dés, et un flux fluide (narration streamée visible token par token).
