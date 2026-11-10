# Un MJ qui travaille avec le joueur — fin du combat forcé, jugement d'intention, et prémisse d'univers

## Intention et contexte

Rapport utilisateur (verbatim) : « vérifie qu'on prenne bien en comptes les
choix de l'utilisateur, j'ai l'impression que le système complet ne fait
que des combats, je veux pouvoir vivre une histoire avec un MJ qui
travaille avec nous, les textes entre * sont normalement des actions que
les joueurs font, de plus je ne peux toujours pas donner les informations
du monde ou de mon personnage ».

Investigation (`systematic-debugging`, root cause confirmé en lisant le
code, pas supposé) :

1. **Combat non évitable** : `amorcer_rencontre_initiale` (dans
   `jouer_tour.rs`) force un combat contre un Gobelin dès le tout premier
   tour de CHAQUE histoire, avant même que l'action du joueur soit
   classifiée. Aucune variante, aucun moyen de l'éviter narrativement.
2. **Aucune action libre en combat n'est respectée** : la correction
   d'hier (« D&D allégé ») a transformé TOUTE action libre non reconnue,
   pendant un combat actif, en tentative d'attaque improvisée — y compris
   « *je négocie* » ou « *j'observe* ». Avant : ignorée mécaniquement.
   Maintenant : toujours interprétée comme une agression. Les deux
   versions ignorent ce que le joueur a réellement exprimé.
3. **Convention `*action*` jamais enseignée au modèle** : le prompt de
   classification dit littéralement « Le joueur dit : "{saisie}" » pour
   N'IMPORTE QUELLE saisie, qu'elle soit entre astérisques ou pas. Rien ne
   distingue un geste décrit d'une réplique parlée.
4. **Pas de prémisse d'univers/personnage** : confirmé en relisant
   `histoires.rs`, `creation.rs`, `worker.rs` — « histoire » ne désigne
   qu'un slot de sauvegarde (personnage + modèle). Il n'existe aucun champ,
   aucun écran, pour que le joueur décrive l'univers ou le contexte qu'il
   veut. Chaque partie démarre de façon identique, à « Clairière », sans
   aucune entrée du joueur au-delà de nom/classe/stats/modèle.

Ces quatre points partagent la même cause racine produit : le jeu ne
laisse aucune place à ce que le joueur exprime, ni au début (prémisse),
ni pendant l'action (intention des actions libres), ni dans son rythme
(combat imposé). Ce document couvre les quatre ensemble, dans un seul
passage sur les mêmes fichiers (décision utilisateur : regrouper plutôt
que scinder).

## Décisions utilisateur (confirmées, pas des suppositions)

- Plus de combat forcé au premier tour : le début est purement narratif ;
  un combat ne démarre que si le joueur agit de façon à en provoquer un,
  ou si le narrateur juge qu'une rencontre hostile survient.
- Pendant un combat, une action libre non reconnue doit être jugée selon
  son CONTENU réel : agressive/physique → attaque improvisée (comme
  avant) ; non-agressive (parler, observer, se cacher, négocier...) →
  résolue narrativement, sans jet d'attaque forcé.
- Les ennemis qui démarrent un combat dynamiquement ont plusieurs paliers
  de difficulté (faible/moyen/fort), pas un seul gabarit générique.
- La fonctionnalité « définir le monde/mon personnage » (texte libre ET
  liste de genres, confirmés hier) est regroupée dans ce même document et
  plan.

## Objectifs

1. Supprimer le déclenchement automatique et inévitable du combat
   d'ouverture.
2. Permettre à un combat de démarrer dynamiquement : soit parce que le
   joueur attaque une cible nommée (dans le récit ou dans sa propre
   saisie), soit parce que le narrateur juge que la situation tourne à
   l'affrontement — avec un gabarit d'ennemi choisi parmi 2-3 paliers de
   difficulté.
3. Pendant un combat, classifier le CONTENU d'une action libre non
   reconnue (agressive ou non) plutôt que de toujours la traiter comme une
   attaque.
4. Enseigner explicitement au modèle la convention `*action*` (geste/
   action du joueur) vs texte nu (parole du joueur), à la fois pour la
   classification et pour la narration.
5. Permettre au joueur de définir, avant de commencer une histoire, un
   genre/thème (liste fermée) ET une courte prémisse libre (texte), que
   le narrateur utilise pour la scène d'ouverture ET garde en tête pendant
   toute l'histoire (pas seulement au premier tour).

## Non-objectifs (hors scope, cohérent avec « D&D allégé »)

- Pas de bestiaire complet par nom de monstre : les paliers de difficulté
  restent génériques (un seul jeu de stats par palier), pas une base de
  données de créatures.
- Pas de système de réputation/relationnel PNJ affecté par le ton des
  actions non-agressives (juste pas de jet d'attaque forcé — la narration
  gère le reste en roleplay, sans nouvel état mécanique).
- Pas de génération d'image/carte, pas de PNJ alliés combattants.
- Pas de validation/modération du contenu de la prémisse libre au-delà de
  ce qui existe déjà (aucune n'existe actuellement — pas ajoutée ici).

## Anti-objectifs (ce qui rendrait ceci un échec même si ça compile)

- Un joueur qui tape une action clairement pacifique pendant un combat et
  se retrouve quand même à attaquer.
- Un joueur bloqué malgré lui dans un combat dès la première action de la
  partie.
- Une prémisse/genre choisi par le joueur mais ignoré après le premier
  tour (le ton doit rester cohérent sur toute la partie, pas juste à
  l'ouverture).
- Perte de rétrocompatibilité silencieuse des sauvegardes existantes
  (un ancien fichier de sauvegarde doit continuer à se charger).

## Décisions techniques (miennes, marquées comme telles)

### A. Suppression du combat forcé, déclenchement dynamique

- Supprimer entièrement `amorcer_rencontre_initiale` et le champ
  `EtatPartie.rencontre_initiale_declenchee` (plus de raison d'être) ainsi
  que son appel inconditionnel en tête de `JouerTour::executer`. Comme le
  champ est actuellement en `#[serde(default)]`, le retirer de la struct
  ne casse pas la lecture des anciennes sauvegardes (`serde_json` ignore
  silencieusement les champs inconnus en désérialisation) — pas besoin de
  shim de migration.
- **Important, vérifié par relecture indépendante** : la génération
  contrainte par grammaire GBNF (`construire_schema_action` dans
  `action_schema.rs`) est `#[allow(dead_code)]` — abandonnée en
  production (crash connu de llama.cpp, voir le commentaire dans
  `llama_narrateur.rs::proposer_action`), gardée seulement pour ses
  propres tests et une éventuelle réactivation future. La classification
  réellement utilisée en production passe par (1) le texte d'instruction
  de `construire_prompt_classification` (`prompts.rs`), envoyé tel quel
  au modèle, et (2) `interpreter_json_action` (le PARSEUR, même fichier
  `action_schema.rs`, mais une fonction bien distincte et elle non
  marquée `dead_code`) qui interprète le JSON librement généré par le
  modèle. **Les nouveaux formats ci-dessous doivent être ajoutés à ces
  deux-là** (`construire_prompt_classification` + `interpreter_json_action`)
  pour avoir un effet réel dans le jeu ; les ajouter seulement à
  `construire_schema_action` (par souci de symétrie avec le code mort,
  optionnel, voir fin de section) n'aurait AUCUN effet sur la partie
  réellement jouée.
- Nouveau format de classification JSON (6ᵉ puis 7ᵉ forme, voir section
  C) : `{"action": "provoquer_combat", "ennemi": "NomDeLEnnemi",
  "difficulte": "faible"|"moyen"|"fort"}`. Le prompt de classification
  instruit le modèle : à utiliser UNIQUEMENT quand aucun combat n'est en
  cours, et seulement si l'action du joueur est agressive envers une
  cible nommée, OU si la situation narrative appelle une rencontre
  hostile soudaine (jugement du narrateur). Ajouter aussi une consigne
  explicite de retenue sur la difficulté : préférer « faible » par
  défaut, réserver « moyen »/« fort » à une situation où le joueur a
  lui-même été notablement agressif ou où l'histoire a déjà clairement
  escaladé — sans cette consigne, un petit modèle local n'a aucune
  raison de ne pas ouvrir sur « fort », ce qui recréerait exactement le
  problème qu'on corrige (un combat difficile et non choisi dès le
  début), juste décidé par le modèle plutôt que câblé en dur. Ceci reste
  une consigne de prompt (un modèle 3B peut l'ignorer), pas une garantie
  mécanique — accepté comme limite inhérente au choix « le narrateur
  peut juger qu'une rencontre survient ».
- Nouveau `domain::ActionJoueur::ProvoquerCombat { ennemi: String,
  difficulte: Difficulte }` (nouvel enum `Difficulte { Faible, Moyen,
  Fort }`, `Serialize`/`Deserialize`/`Clone`/`Copy`/`PartialEq`/`Eq`).
- Nouvelle fonction `domain::gabarit_ennemi(nom: String, difficulte:
  Difficulte) -> Ennemi`, avec ces trois paliers (mon choix, le palier
  « Moyen » reprend exactement les stats de l'ancien Gobelin pour ne pas
  changer la difficulté perçue par défaut) :

  | Palier | `pv_max` | `degats` | `defense` | `bonus_initiative` | `bonus_attaque` |
  |---|---|---|---|---|---|
  | Faible | 4 | 1d4 | 10 | 1 | 1 |
  | Moyen | 7 | 1d6 | 12 | 2 | 3 |
  | Fort | 12 | 1d8 | 14 | 3 | 5 |

- `valider_action` : `ProvoquerCombat` est toujours valide (pas de
  précondition — c'est justement l'action qui FAIT démarrer le combat).
- Résolution (nouveau bras dans `resoudre_action_joueur`) :
  - Si `combat_en_cours.is_none()` : construit l'`Ennemi` via
    `gabarit_ennemi`, appelle `commencer_combat` (comme avant pour le
    Gobelin, même fonction, juste avec cet ennemi), affecte
    `etat.combat_en_cours`, pousse un message de résumé (« ⚔️ {ennemi}
    surgit et t'attaque ! », formulation reprise de l'ancien message pour
    rester cohérente). L'ennemi ainsi créé n'agit qu'au tour SUIVANT
    (laissé tel quel par simplicité — `ordre_de_tour` n'est calculé
    qu'une fois en tête de `executer`, avant que ce bras ne s'exécute ; un
    ennemi avec une initiative plus rapide qui attaquerait DANS LE MÊME
    tour où il apparaît est une amélioration possible mais pas nécessaire
    ici, laissée au bon sens du développeur si le temps le permet).
  - Si un combat est déjà en cours (le modèle s'est trompé de catégorie
    alors qu'on y est déjà) : retomber sur le même comportement que
    `Attaquer { cible: ennemi }` par défense (ne pas faire planter le
    tour pour une mauvaise classification).
  - **Générateur de dés** : `resoudre_action_joueur` ne reçoit
    aujourd'hui qu'un seul `de: &mut dyn GenerateurDe` (vérifié par
    relecture indépendante — c'est délibérément `de_joueur`, réservé aux
    jets qui appartiennent au joueur ; `commencer_combat` utilise
    `de_monde` pour l'initiative, y compris celle de l'ancien Gobelin).
    Le nouvel ennemi dynamique doit avoir son initiative tirée par
    `de_monde`, pas `de_joueur` — `resoudre_action_joueur` gagne donc un
    second paramètre `de_monde: &mut dyn GenerateurDe`, utilisé
    UNIQUEMENT par ce bras (`ProvoquerCombat`, via `commencer_combat`).
    Mettre à jour son unique appelant dans `JouerTour::executer`, où
    `de_monde` est déjà disponible.

### B. Jugement d'intention pour les actions libres en combat

- `domain::ActionJoueur::ActionLibre` passe de `ActionLibre(String)` à
  `ActionLibre { texte: String, agressif: bool }`. `agressif` n'a de sens
  que pendant un combat ; hors combat, toujours `false` côté
  classification (valeur ignorée par la résolution hors combat, qui reste
  purement narrative comme avant).
- Comme pour la section A : le changement réel se fait dans
  `interpreter_json_action` (le bras `"libre"`, aujourd'hui
  `let texte = ...; Ok(ActionJoueur::ActionLibre(texte))` — lire aussi le
  champ `"agressif"`, défaut `false` si absent) et dans
  `construire_prompt_classification` (`prompts.rs`). `construire_schema_action`
  (code mort, `action_schema.rs`) peut être étendu en parallèle par
  souci de cohérence avec son propre commentaire (« gardée pour une
  future réactivation »), mais ce n'est pas ce qui corrige le
  comportement du jeu réellement joué.
- **Repli sur sortie JSON non reconnue** : `LlamaNarrateur::proposer_action`
  (vérifié par relecture indépendante) retombe aujourd'hui sur
  `ActionLibre(String::new())` dès que `interpreter_json_action` échoue
  (JSON invalide, ou action hors du schéma connu — ex. observé en test
  réel : `{"action": "préparer_sort"}`), puis réinjecte la saisie
  verbatim du joueur par-dessus. C'est précisément le chemin le plus
  probable de se reproduire avec un petit modèle local pendant un
  combat. Une fois `ActionLibre` étendu avec `agressif`, ce repli doit
  utiliser **`agressif: false`** — un défaut à `true` recréerait
  silencieusement le bug exact que ce document corrige (toute sortie mal
  formée pendant un combat redeviendrait une attaque forcée).
- Prompt de classification (`construire_prompt_classification`) : ajouter
  une consigne explicite quand un combat est actif, expliquant que
  `agressif: true` signifie une action physiquement hostile (frapper,
  jeter quelque chose, charger...) et `agressif: false` une action non
  violente (parler, négocier, observer, se cacher, fuir discrètement sans
  combat...).
- Résolution (`resoudre_action_joueur`, bras `ActionLibre`) :
  - `combat_actif && agressif` : comportement actuel inchangé (attaque
    improvisée avec désavantage, jet réel, dégâts réels).
  - `combat_actif && !agressif` : AUCUN jet, aucun dégât — pousser un
    message de résumé neutre (ex. `"💬 Tentative non-agressive : « {texte}
    »."`) pour que la narration downstream sache qu'il ne s'est rien
    passé de mécanique, mais que l'action a bien existé (évite de
    retomber dans le bug original « aucune trace de ce que le joueur a
    fait »).
  - Hors combat : inchangé (purement narratif, aucun jet).

### C. Convention `*action*` vs parole

- `construire_prompt_classification` ET `construire_prompt_narration`
  gagnent chacun une ligne d'instruction explicite (texte exact laissé au
  développeur, sens requis) : le texte entouré d'astérisques (`*...*`)
  décrit un GESTE ou une ACTION physique du joueur ; le texte sans
  astérisques est ce que le joueur DIT à voix haute. Les deux doivent être
  pris en compte pour classifier/narrer, mais la nature (action vs
  parole) doit être respectée dans la narration produite (ex. ne pas
  mettre entre guillemets de dialogue une action purement physique).
- Pas de traitement spécial côté Rust (pas de strip des astérisques, pas
  de parsing) — c'est une instruction au modèle, pas une transformation
  de données, pour rester robuste à toute formulation (le joueur peut
  très bien ne jamais utiliser d'astérisque, ou mélanger les deux dans un
  même message).

### D. Prémisse d'univers et de personnage

- Nouveau `domain::Genre` (`Serialize`/`Deserialize`/`Clone`/`Copy`/
  `PartialEq`/`Eq`), mon choix de liste fermée (modifiable facilement
  plus tard, c'est juste un enum) :
  `FantasyMedievale`, `ScienceFiction`, `Horreur`, `PolarNoir`,
  `PostApocalyptique`, `Steampunk`. Chaque variant porte un libellé
  affichable (`nom_affiche()`, même motif que `PaletteModele` dans
  `app_desktop/src/modeles.rs`) et une courte description à injecter dans
  le prompt d'introduction (ex. `ScienceFiction.indication_prompt()` →
  "un futur de vaisseaux spatiaux et de technologies avancées").
- **Longueur de la prémisse libre** : le `<textarea>` reçoit un
  `maxlength` (mon choix : 300 caractères — assez pour un vrai
  paragraphe, assez court pour ne pas dominer le budget de tokens d'un
  modèle local déjà sous contrainte, voir `SEUIL_MOTS_RESUMES`/
  `condenser_si_necessaire`). Défense en profondeur (motif déjà établi
  dans ce projet) : tronquer aussi à la construction du prompt
  (`description_premisse`), pas seulement côté UI, pour couvrir toute
  donnée plus ancienne ou tout futur appelant qui ne passerait pas par
  cet écran.
- Nouveau `domain::PremisseHistoire { genre: Genre, description: String
  }` — `description` peut être une chaîne vide (= aucune prémisse libre,
  le genre seul suffit à orienter le ton ; combinaison des deux options
  validées hier : liste de genres ET texte libre, les deux optionnels
  indépendamment, un genre par défaut raisonnable choisi dans l'UI évite
  d'avoir un état "rien du tout").
- `EtatPartie` gagne `pub premisse: PremisseHistoire` (pas `Option<...>` :
  un genre est toujours choisi, avec un défaut sensé dans l'UI — évite un
  cas `None` à gérer partout en aval). Comme c'est un NOUVEAU champ non
  optionnel, les anciennes sauvegardes sans ce champ échoueraient à la
  désérialisation sans `#[serde(default)]` : ajouter
  `#[serde(default)]` avec un `Default` raisonnable pour `PremisseHistoire`
  (ex. `FantasyMedievale` + description vide), pour rester cohérent avec
  le motif déjà utilisé sur `pnj_connus`/`rencontre_initiale_declenchee`
  — une ancienne sauvegarde se relit avec une prémisse par défaut plutôt
  que d'échouer à charger.
- `construire_prompt_introduction` : nouvelle signature acceptant la
  `PremisseHistoire` (en plus de `personnage`/`lieu`), injectée dans le
  prompt pour orienter la scène d'ouverture (genre + description libre si
  non vide).
- `construire_prompt_narration` : ajoute un rappel court de la prémisse
  (nouvelle fonction d'aide, même motif que `description_personnage`/
  `description_pnj_pertinents`) à CHAQUE tour, pas seulement à
  l'ouverture — un petit modèle local (Qwen2.5-3B) n'a pas de mémoire
  longue, le ton doit être rappelé pour rester cohérent sur toute la
  partie.
- UI (`app_desktop/src/vues/creation.rs`) : ajoute, sur le même écran de
  création déjà existant (pas un nouvel écran séparé — motif déjà
  utilisé pour le choix du modèle), un `<select>` pour le genre (liste
  statique, même motif que Classe/Modèle) et un `<textarea>` pour la
  description libre (optionnelle, vide par défaut). `MessageVersWorker::
  CreerPersonnage` gagne `genre: Genre, premisse_libre: String`.
  `worker::creer_nouvelle_histoire` les reçoit et construit
  `EtatPartie.premisse` avec.

## Portée mécanique connue

Retirer `rencontre_initiale_declenchee` et ajouter `premisse` change la
forme de `EtatPartie` : chaque construction littérale de `EtatPartie`
dans le code (tests et fixtures, pas de logique) doit être mise à jour
en conséquence. Vérifié par recherche : 11 fichiers concernés
(`crates/app_desktop/src/worker.rs`, `crates/application/src/ports/
sauvegarde.rs`, `crates/application/src/usecases/jouer_tour.rs`,
`crates/application/src/usecases/partie.rs`, `crates/app/src/main.rs`,
`crates/domain/src/etat_partie.rs`, `crates/infra/src/action_schema.rs`,
`crates/infra/src/faux_narrateur.rs`, `crates/infra/src/
llama_narrateur.rs`, `crates/infra/src/prompts.rs`, `crates/infra/src/
sauvegarde_fichier.rs`) — même genre de balayage mécanique que pour
`ennemi_agit_avant_joueur` lors d'une session précédente (script
bottom-to-top par fichier, pas de surprise attendue, mais à ne pas
sous-estimer dans le plan).

## Ce qui reste au développeur

- Formulation exacte des lignes de prompt (C, D) — le sens requis est
  fixé ci-dessus, la phrase française exacte est un détail
  d'implémentation.
- Libellés affichés exacts des genres dans l'UI (`nom_affiche()`) et
  ordre de la liste déroulante.
- Message de résumé exact pour une action libre non-agressive en combat
  (B) — le sens et l'absence de jet sont fixés, la formulation est libre.
- Si le temps le permet, faire agir un ennemi nouvellement apparu
  (`ProvoquerCombat`) dans le même tour quand son initiative dépasse
  celle du joueur, plutôt que d'attendre le tour suivant (amélioration
  non bloquante, voir section A).
