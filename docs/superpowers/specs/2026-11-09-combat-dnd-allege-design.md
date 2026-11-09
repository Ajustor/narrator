# Combat « D&D allégé » — corriger les actions ignorées et rapprocher les règles de D&D

## Intention et pourquoi

Rapport utilisateur : pendant un combat, les actions du joueur « ne sont pas
bien prises en compte », et il demande d'appliquer les règles de Donjons et
Dragons au combat.

Diagnostic (confirmé en lisant `jouer_tour.rs`, `action_schema.rs`,
`prompts.rs`) : le classificateur ne reconnaît que quatre formes —
`attaquer`, `utiliser_objet`, `fuir`, `libre`. Tout texte créatif du joueur en
combat qui n'est pas une attaque/un objet/une fuite littérale tombe dans
`libre`, qui **n'a aucun effet mécanique en combat** : pas de jet, pas de
dégâts, rien — même si le récit généré décrit une action. L'ennemi, lui,
continue d'attaquer normalement à son tour. C'est très probablement ce que
le joueur ressent comme « mes actions ne comptent pas ».

Choix de portée (validé par l'utilisateur via question à choix) : **D&D
allégé**, sans capacités de classe ni sorts — ceux-ci restent une extension
future si demandée.

## Objectifs

1. Plus aucune action du joueur en combat n'est ignorée mécaniquement :
   toute action texte libre en combat devient une attaque improvisée réelle
   (jet, dégâts) au lieu d'un no-op.
2. Bonus de maîtrise (+2) ajouté au jet d'attaque du joueur (pas aux dégâts),
   cohérent avec un personnage de niveau 1 maîtrisant son arme.
3. Critique : un 20 naturel touche toujours et double les dés de dégâts
   (relancés, pas doublés arithmétiquement). Échec critique : un 1 naturel
   rate toujours, même si le total aurait suffi.
4. Une vraie action **Esquiver** : le joueur peut choisir de se défendre ;
   jusqu'au début de son tour suivant, les attaques ennemies contre lui sont
   faites avec désavantage (deux jets, on garde le plus bas).
5. Ordre d'initiative par combattant quand il y a plusieurs ennemis, au lieu
   d'un simple booléen « les ennemis agissent avant/après » qui groupait
   tous les ennemis ensemble.

## Non-objectifs

- Capacités ou sorts spécifiques à une classe (classe reste une étiquette
  narrative).
- Conditions (à terre, étourdi…), jets de sauvegarde, attaques multiples par
  tour, emplacements de sorts — tier supérieur, pas demandé maintenant.
- Nouveaux boutons d'UI pour Esquiver : comme `Attaquer`/`Fuir` aujourd'hui,
  c'est détecté depuis le texte libre du joueur par le classificateur, pas
  un bouton dédié.
- Rééquilibrer les ennemis existants (`bonus_attaque` du Gobelin reste tel
  quel : il joue déjà le rôle d'un bonus d'attaque complet, pas d'un
  modificateur brut à ajouter à une maîtrise séparée).

## Anti-objectifs

- Ne pas casser la compatibilité des sauvegardes existantes (`#[serde(default)]`
  partout où c'est pertinent, déjà le pattern établi dans ce code).
- Ne pas changer l'architecture à deux générateurs de dés (`de_joueur`/`de_monde`)
  ni le protocole worker↔UI pour le dé virtuel — les nouveaux jets
  (désavantage, dégâts critiques) sont juste des appels supplémentaires à
  `GenerateurDe::lancer`, pas un nouveau mécanisme.

## Décisions techniques (mes choix, pas à valider par l'utilisateur)

- **Désavantage** = appeler `.lancer()` deux fois avec le même type de dé et
  modificateur, garder le total le plus bas. Pour le joueur, ça veut dire
  deux jets successifs sur le dé virtuel à l'écran (même étiquette
  "Jet d'attaque" affichée deux fois d'affilée) — pas d'étiquette spéciale
  "désavantage 1/2" pour l'instant : changer l'étiquette demanderait de faire
  passer un contexte supplémentaire à travers `GenerateurDeJoueur`, hors
  scope pour ce lot.
- **Dégâts critiques** = relancer le dé de dégâts une deuxième fois et
  sommer les deux résultats bruts (modificateur appliqué une seule fois,
  comme en D&D 5e), pas multiplier le total par 2.
- **Cible de l'attaque improvisée** (action libre en combat) = le premier
  ennemi vivant. Le jeu ne spawn actuellement jamais plus d'un ennemi à la
  fois, donc l'ambiguïté ne se manifeste pas encore en jeu ; le choix est
  documenté pour quand ça changera.
- **Esquiver et le découpage en « un tour = un appel »** : le flag
  `joueur_esquive` sur `Combat` est mis à `true` quand l'action Esquiver se
  résout, s'applique à toutes les attaques ennemies rencontrées ensuite dans
  l'ordre d'initiative (y compris celles du tour suivant si des ennemis plus
  rapides agissent avant la prochaine action du joueur), et n'est remis à
  `false` qu'au moment où le joueur agit à nouveau — ce qui reproduit
  fidèlement « jusqu'au début de ton tour suivant » malgré le fait que le
  jeu ne résout qu'un tour complet par appel.
- **Ordre d'initiative** : remplace `ennemi_agit_avant_joueur: bool` par
  `initiative_joueur: i32` + `initiatives_ennemis: Vec<i32>` (même index que
  `ennemis`) sur `Combat`. Une méthode `Combat::ordre_de_tour()` construit la
  séquence complète (chacun trié par initiative décroissante, égalité
  tranchée en faveur du joueur). Résoudre un tour consiste à itérer cette
  séquence, en s'arrêtant dès que le combat se termine (victoire, fuite, ou
  mort du joueur) plutôt que de résoudre « tous les ennemis d'un bloc ».
  Sauvegardes existantes : anciens champs ignorés silencieusement par
  `serde` (non strict), nouveaux champs avec `#[serde(default)]` — une
  sauvegarde rechargée en plein combat avec des longueurs incohérentes
  retombe sur un ordre « joueur d'abord », sans paniquer (cas limite
  accepté : un combat est toujours très court dans ce jeu solo).
- **Nat 1 / nat 20** : lus depuis `JetDe.resultat` (le jet brut avant
  modificateur), pas depuis `total`.

## Ce qui reste à l'implémenteur (moi-même, inline, pas de sous-agents — les
tâches partagent trop de code pour être indépendantes)

- Écrire/adapter les tests avant le code (TDD), fichier par fichier, dans cet
  ordre de dépendances : `domain::combat` (évaluation touché/critique,
  `ordre_de_tour`) → `domain::action_joueur` (variant `Esquiver`) →
  `application::commencer_combat` → `application::jouer_tour` (le plus gros
  morceau : boucle de résolution par ordre d'initiative, attaque improvisée,
  esquive) → `infra::action_schema` + `infra::prompts` (nouveau format JSON
  "esquiver") → mise à jour mécanique des ~18 littéraux `EtatPartie`/`Combat`
  existants dans les autres crates (même sweep que pour
  `rencontre_initiale_declenchee` précédemment).
- `cargo test --workspace` doit passer à la fin, sans warning.
- Vérification manuelle (console `app`, suffisant pour ce lot — pas besoin
  de rebuild APK sauf demande explicite ensuite) : un combat où le joueur
  tape une action créative et voit un vrai jet + des dégâts réels ; un combat
  à 2 ennemis via un test (pas besoin de contenu de jeu dédié) prouvant
  l'ordre par combattant ; esquiver puis constater le désavantage appliqué.
