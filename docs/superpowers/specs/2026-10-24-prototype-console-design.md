# Design — Prototype console (étape 1 de la feuille de route)

Date : 2026-10-24
Auteur : Alexandre Gambier (avec Claude)
Contexte : premier jalon de *Narrateur - vivez votre histoire*. Voir le brief projet pour la vision, le périmètre MVP et la feuille de route complète.

## Objectif

Construire, sur desktop, une crate `core` (découpée en `domain` + `application` + `infra`) et un binaire CLI minimal (`app`) qui font tourner une boucle de JDR solo jouable dans un terminal :

- narration pilotée par un faux MJ (pas encore de vrai LLM),
- un combat au tour par tour complet (initiative, PV, dégâts),
- jets de dés déterministes et testables,
- sauvegarde/reprise de partie en JSON.

L'intégration réelle du LLM (`llama-cpp-2`) est explicitement **hors scope** de cette étape — elle arrive une fois la boucle validée, derrière le même port `Narrateur`. Ce choix diffère légèrement du texte de la feuille de route (qui mentionne l'appel LLM dès l'étape 1) : on a décidé de dérisquer la boucle de jeu d'abord, pour ne pas bloquer sur la compilation de llama.cpp avant même d'avoir un moteur qui fonctionne.

## Hors scope (reporté aux étapes suivantes)

- Vrai modèle LLM / `llama-cpp-2` (étape 2).
- Résumés automatiques de mémoire / fiches PNJ (étape 2).
- Interface Dioxus, assistant de création de personnage graphique (étape 3).
- Portage Android (étape 4).
- Thème visuel, plusieurs campagnes (étape 5).

## Architecture — workspace Cargo

```text
narrateur/
├── Cargo.toml                  # workspace + profil release (voir brief)
└── crates/
    ├── domain/                 # entités + règles pures (serde seulement)
    ├── application/            # ports (traits) + cas d'usage
    ├── infra/                  # implémentations des ports
    └── app/                    # binaire CLI du prototype
```

Règle de dépendance stricte : `domain` ne dépend de rien d'externe (hormis `serde`). `application` dépend de `domain` et définit des traits, sans connaître leurs implémentations. `infra` dépend de `application` + `domain` et implémente les traits. `app` assemble tout.

## Domaine (`domain`)

Entités et value objects, toutes les méthodes de résolution sont pures (pas d'I/O) :

- `Personnage` { nom, classe, caractéristiques, pv_actuels, pv_max, inventaire: Vec<Objet> }
- `Caracteristiques` { force, dexterite, constitution, intelligence, sagesse, charisme } + calcul de modificateur
- `Objet` { nom, description, effet: Option<EffetObjet> } — `EffetObjet::Arme{degats: De}` ou `EffetObjet::Consommable{soin: u32}`
- `Ennemi` { nom, pv_actuels, pv_max, degats: De, bonus_initiative }
- `Combat` { participants ordonnés par initiative, tour_actuel, terminé: bool, vainqueur: Option<Vainqueur> }
- `De` { faces: u32 }, `JetDe` { resultat, modificateur, total }
- `ActionJoueur` (enum) { Attaquer{cible}, UtiliserObjet{objet}, Fuir, ActionLibre(String) }
- `EtatPartie` { personnage, lieu_actuel: String, combat_en_cours: Option<Combat>, resumes: Vec<String> } — sérialisable, c'est tout l'état persistant

Règles vivant en méthodes sur ces types : modificateur de caractéristique, résolution d'une attaque (jet + seuil + dégâts), mort du personnage, fin de combat (victoire/défaite/fuite).

## Ports (`application`)

```rust
trait Narrateur: Send {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur>;
}

trait GenerateurDe {
    fn lancer(&mut self, de: De) -> JetDe;
}

trait Sauvegarde {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde>;
    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde>;
}
```

`ReponseMj` contient la narration (texte) et une `ActionJoueur` proposée, extraite du bloc JSON renvoyé par le narrateur. Une implémentation seedée de `GenerateurDe` (RNG déterministe) vit dans `application` derrière `#[cfg(test)]`, pour que les tests des cas d'usage soient reproductibles sans dépendre d'`infra`.

## Cas d'usage (`application`)

- `CreerPersonnage` : valide nom/classe/répartition de caractéristiques (budget de points) et construit un `Personnage`.
- `CommencerCombat` : construit un `Combat` à partir d'un ou plusieurs `Ennemi`, tire l'initiative via `GenerateurDe`.
- `JouerTour` : cas d'usage central.
  1. Appelle `Narrateur::raconter` avec l'état courant + la saisie libre du joueur → récupère narration + `ActionJoueur` proposée.
  2. Valide l'action contre les règles (objet présent en inventaire ? cible existante et vivante ?).
  3. Résout l'action du joueur via `GenerateurDe` (jet, dégâts) — **le LLM ne calcule jamais de nombre**.
  4. Si un combat est en cours, résout l'action de l'ennemi **déterministement en code** (IA simple : attaque la cible la plus faible) — pas d'appel LLM pour l'ennemi.
  5. Met à jour `EtatPartie`, retourne la narration + un résumé mécanique du tour (jets/dégâts), affichés séparément dans le terminal.
- `SauvegarderPartie` / `ChargerPartie` : wrappent le port `Sauvegarde`.

Décision clé : seule l'action du **joueur** passe par une interprétation LLM (texte libre → `ActionJoueur`). Les ennemis ne sont jamais pilotés par le LLM, pour garder les combats déterministes et reproductibles.

## Implémentations (`infra`)

- `FauxNarrateur` : implémente `Narrateur` par repérage de mots-clés dans la saisie (ex: "attaque" → `Attaquer`, "potion"/"soigne" → `UtiliserObjet`, "fuir" → `Fuir`, sinon → `ActionLibre`) + narration template. Permet de jouer toute la boucle en terminal sans modèle réel.
- `GenerateurDeOs` : implémente `GenerateurDe` avec `rand::rng()` (non-déterministe), utilisé par `app`.
- `SauvegardeFichier` : implémente `Sauvegarde`, sérialise `EtatPartie` en JSON (`serde_json`) vers un fichier local (`./partie.json` pour le prototype).

## `app` — boucle CLI

Démarrage : si `./partie.json` existe, `SauvegardeFichier::charger` reprend la partie ; sinon, `CreerPersonnage` en interactif (prompts `stdin`), puis `CommencerCombat` avec un encounter fixe (un gobelin) pour valider la boucle de bout en bout.

Boucle principale, par tour :
1. Afficher la narration courante, lire une ligne de saisie libre.
2. `JouerTour` — le callback `sur_fragment` imprime au fil de l'eau (valide le contrat du trait même si `FauxNarrateur` ne produit pas de vrai streaming token-par-token).
3. Afficher le résumé mécanique du tour séparément de la prose.
4. Sauvegarder l'état après chaque tour (`SauvegardeFichier::sauvegarder`).
5. Si combat terminé (victoire/mort/fuite) : afficher le dénouement et quitter.

## Stratégie de test

- `domain` : tests unitaires purs sur les méthodes de résolution — aucune I/O.
- `application` : tests des cas d'usage (`JouerTour`, `CommencerCombat`) avec le `GenerateurDe` seedé → scénarios de combat déterministes.
- `infra` : test d'intégration léger pour `SauvegardeFichier` (écrire/relire un `EtatPartie`).
- `app` : pas de test automatisé pour la boucle CLI (I/O interactive) — validation manuelle en jouant une partie.

## Critères de complétion de l'étape 1

- `cargo test` passe sur tout le workspace.
- Jouer une partie complète en terminal : création de personnage → combat contre un gobelin (victoire ou mort) → sauvegarde → relance du binaire → reprise de la partie sauvegardée.
