# Design — App Dioxus desktop

Date : 2026-10-26
Auteur : Alexandre Gambier (avec Claude)
Contexte : étape 3 de la feuille de route ("App Dioxus desktop"). L'étape 2 ("Prompts et mémoire" — intégration LLM réelle, résumés automatiques, fiches PNJ) est entièrement terminée et fusionnée dans `master`. Cette étape ajoute une interface graphique de bureau au-dessus du même moteur de jeu (`domain`/`application`/`infra`, inchangés), sans toucher à la boucle console existante.

## Objectif

Donner au jeu une interface graphique de bureau (Dioxus) équivalente à la boucle console actuelle — création de personnage, narration en direct (streaming), combat, sauvegarde automatique — plus la consultation des fiches PNJ, explicitement remise à cette étape dans la spec des fiches PNJ (« Toute UI de consultation des fiches PNJ (réservé à l'étape Dioxus) »).

## Hors scope

- Portage Android (étape 4 de la feuille de route — réservé).
- Retrait ou modification de l'app console (`crates/app`) — elle reste disponible telle quelle, inchangée.
- Toute mécanique de jeu nouvelle (rien dans `domain`/`application`/`infra` ne change pour cette étape — seule une nouvelle couche `app` est ajoutée).
- Affinage des threads pour cœurs performants/efficaces (déjà noté hors scope à l'étape Android dans la spec LLM-narrateur).
- Correspondance floue des PNJ dans l'injection narrative (déjà hors scope de la spec fiches PNJ, inchangé ici).

## Décision d'architecture clé : un thread dédié possède toute la boucle de jeu

La génération LLM (`LlamaNarrateur::generer`) est bloquante et synchrone — potentiellement plusieurs dizaines de secondes par tour. Dioxus a besoin que son thread d'interface reste libre pour rester réactif (effet machine à écrire en direct inclus). Plutôt que de déplacer `LlamaNarrateur` à travers des frontières async par appel, **un thread système dédié possède `EtatPartie`, `LlamaNarrateur`, `GenerateurDeOs` et `SauvegardeFichier` pendant toute la durée de vie de l'application** — c'est presque exactement la boucle de `crates/app/src/main.rs` actuelle, avec stdin/stdout remplacés par des canaux. Le modèle ne traverse donc jamais de frontière de thread après son chargement initial, exactement comme dans la console.

Deux canaux relient ce thread au thread Dioxus :

- **UI → thread de jeu** : `std::sync::mpsc::channel::<MessageVersWorker>()`. Le thread de jeu est un `std::thread` classique (pas async) ; `.recv()` bloquant en boucle est le point d'entrée naturel.
- **Thread de jeu → UI** : `futures_channel::mpsc::unbounded::<MessageVersUi>()`. L'émission (`.unbounded_send(...)`) est non bloquante et appelable depuis n'importe quel thread ; la réception côté Dioxus se fait dans une tâche asynchrone lancée avec `use_future` (s'exécute sur le même thread que le reste de l'UI Dioxus, donc peut muter les signaux directement), qui boucle sur `.next().await` et met à jour les signaux au fur et à mesure.

```rust
pub enum MessageVersWorker {
    CreerPersonnage { nom: String, classe: String, valeurs: [u32; 6] },
    JouerTour(String),
    NouvellePartie,
}

pub enum MessageVersUi {
    EtatInitial(Option<EtatPartie>),
    PartieCreee(EtatPartie),
    NarrationFragment(String),
    TourTermine { etat: EtatPartie, resultat: ResultatTour },
    TourEchoue(String),
    PartieReinitialisee,
}
```

Le thread de jeu reprend la logique de `main()` actuelle : au démarrage, tente `charger_partie` et envoie `EtatInitial(Some(etat))` ou `EtatInitial(None)` ; sur `CreerPersonnage`, appelle `application::usecases::creer_personnage::creer_personnage` (la fonction pure déjà utilisée par la création console — pas l'enrobage interactif `creation::creer_personnage_interactif`, qui lit stdin et n'est pas repris) puis construit le combat initial comme `main()` le fait aujourd'hui, et envoie `PartieCreee(etat)` ; sur `JouerTour(saisie)`, appelle `JouerTour::executer` avec un callback qui envoie un `NarrationFragment` par fragment reçu, puis envoie `TourTermine{etat, resultat}` ou `TourEchoue(message)` ; sur `NouvellePartie`, supprime la sauvegarde et envoie `PartieReinitialisee`.

## Vues / routage

Pas de vrai routeur (`dioxus-router`) — un `enum Vue` dans un signal, matché pour choisir le composant actif :

```rust
pub enum Vue {
    Creation,
    Jeu,
    FinDePartie(Vainqueur),
    ErreurChargement(String),
}
```

- **`ErreurChargement`** : affiché si le thread de jeu échoue à charger le modèle (variable `NARRATEUR_MODELE` absente/invalide) — message d'erreur visible dans la fenêtre plutôt qu'un `eprintln!`/`exit(1)` muet côté GUI. Pas de retentative pour ce premier passage.
- **`Creation`** : affiché si `EtatInitial(None)` est reçu (aucune sauvegarde). Champ nom, select classe (`CLASSES_VALIDES`), six select pour les caractéristiques — chacun ne proposant que les valeurs encore disponibles de `VALEURS_STANDARD` (même contrainte que `creation::lire_caracteristiques` côté console). Bouton « Commencer l'aventure » → envoie `CreerPersonnage{...}`.
- **`Jeu`** : affiché si `EtatInitial(Some(etat))` ou `PartieCreee(etat)` est reçu. Détaillé ci-dessous.
- **`FinDePartie`** : affiché quand `TourTermine` porte un `resultat.combat_termine == true`. Message de victoire/défaite/fuite (repris du `match` existant dans `main.rs`). Bouton « Nouvelle partie » → envoie `NouvellePartie`, puis sur `PartieReinitialisee` retourne à `Vue::Creation`.

## Vue Jeu — disposition

- **En-tête** : `etat.lieu_actuel`, PV du joueur (`pv_actuels/pv_max`).
- **Panneau combat** : visible seulement si `etat.combat_en_cours` est `Some` et non terminé — liste des ennemis vivants avec leurs PV (mêmes données que `description_etat_apres_resolution` calcule déjà côté prompt, affichées plutôt qu'injectées en texte).
- **Zone de narration** : journal défilant des narrations de la session en cours (pas besoin de réafficher `resumes`/l'historique complet — la console elle-même n'affiche jamais plus que ce qui a défilé dans le terminal). Pendant la génération, la narration en cours s'écrit progressivement : un signal `narration_en_cours: Signal<String>` est vidé au début de chaque tour puis complété fragment par fragment (`narration_en_cours.write().push_str(&fragment)`) à chaque `MessageVersUi::NarrationFragment` reçu ; à `TourTermine`, le contenu final est ajouté au journal et `narration_en_cours` est vidé.
- **Zone de saisie** : champ texte + bouton (ou touche Entrée) pour envoyer `JouerTour(saisie)`. Désactivée tant qu'un tour est en cours (entre l'envoi et la réception de `TourTermine`/`TourEchoue`) — pas de double-envoi pendant la génération.
- **Panneau PNJ** : panneau latéral repliable listant `etat.pnj_connus` (nom + description), chaque entrée cliquable pour déplier son `historique` complet. Mis à jour directement depuis l'`EtatPartie` reçu dans chaque `TourTermine`/`PartieCreee`/`EtatInitial` — pas de requête séparée.

## Gestion des erreurs

- Échec de chargement du modèle → `Vue::ErreurChargement(message)`, voir ci-dessus.
- `MessageVersUi::TourEchoue(message)` → ajouté comme ligne d'avertissement dans le journal de narration (pas de popup bloquant), le champ de saisie se réactive immédiatement.

## Fichiers (`crates/app_desktop`, nouveau crate)

- `Cargo.toml` : dépendances `dioxus = { version = "0.7", features = ["desktop"] }` (le numéro de version exact et le nom du feature flag sont à vérifier contre le `Cargo.lock` réel au moment de l'implémentation — l'écosystème Dioxus évolue vite), `futures-channel`, plus `domain`/`application`/`infra` en dépendances de chemin (comme `crates/app`).
- `src/protocole.rs` : `MessageVersWorker`, `MessageVersUi`, `Vue`.
- `src/worker.rs` : la boucle de jeu décrite ci-dessus — une fonction `lancer(rx: std::sync::mpsc::Receiver<MessageVersWorker>, tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>, chemin_modele: PathBuf)` destinée à tourner sur son propre `std::thread::spawn`.
- `src/vues/creation.rs`, `src/vues/jeu.rs`, `src/vues/fin_de_partie.rs`, `src/vues/erreur_chargement.rs` : un composant Dioxus par écran.
- `src/main.rs` : lit `NARRATEUR_MODELE`, crée les deux canaux, lance le thread de jeu (`worker::lancer`), appelle `dioxus::launch(App)` où `App` lance la tâche `use_future` qui draine le canal `MessageVersUi` et câble les signaux/la vue active.

## Stratégie de test

- Le rendu Dioxus lui-même n'est pas testé automatiquement — comme `LlamaNarrateur`, vérifié manuellement en lançant l'app (voir Critères de complétion). Aucun précédent dans ce projet de test automatisé d'une UI graphique.
- La logique du thread de jeu (`worker.rs`) est extraite dans une fonction testable indépendante des canaux réels — une fonction `traiter_message(etat: &mut Option<EtatPartie>, message: MessageVersWorker, narrateur: &mut dyn Narrateur, de: &mut dyn GenerateurDe, sauvegarde: &dyn Sauvegarde, sur_message_ui: &mut dyn FnMut(MessageVersUi)) -> ()`, testable avec les doubles déjà existants dans `application`/`infra` (`FauxNarrateur`, `GenerateurDeSeed`, un double `Sauvegarde` en mémoire) — même esprit que les tests existants de `JouerTour`. Couvre : création de personnage, tour réussi (fragments + résultat), tour en échec, fin de partie (victoire/défaite/fuite), nouvelle partie.
- `src/protocole.rs` (types purs, pas de logique) n'a pas besoin de tests dédiés.

## Critères de complétion

- `cargo test --workspace` passe (les nouveaux tests de `worker.rs`, plus tout ce qui existe déjà).
- `cargo build -p app_desktop` compile.
- Vérification manuelle, avec `NARRATEUR_MODELE` pointant vers un modèle réel : lancer l'app, créer un personnage, jouer plusieurs tours hors combat et en combat (narration en direct visible, panneaux PV/combat à jour), rencontrer un PNJ et vérifier qu'il apparaît dans le panneau PNJ, terminer une partie (victoire/défaite/fuite) et en recommencer une, et provoquer une erreur de chargement (variable d'environnement absente) pour vérifier l'écran `ErreurChargement`.
