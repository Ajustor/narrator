# Design — Plusieurs histoires et choix du modèle par histoire

Date : 2026-11-08
Auteur : Alexandre Gambier (avec Claude)

## Intention et pourquoi

En testant le jeu pour de vrai (desktop et Android), deux limites structurelles empêchent d'y « vraiment jouer » :

1. Le combat termine toujours la partie entière (corrigé séparément, commit `0ab093f` — la victoire/fuite remet le jeu en exploration libre au lieu de tout terminer).
2. Il n'existe qu'**une seule sauvegarde possible** (`partie.json`, écrasée à chaque nouvelle partie) et **un seul modèle possible** (fixé par `NARRATEUR_MODELE` sur desktop, toujours le même modèle téléchargé sur Android). Impossible d'avoir deux histoires en parallèle, impossible de choisir un modèle plus léger (plus rapide sur mobile) ou plus costaud (meilleure narration) selon l'histoire.

Ce document couvre le point 2 : plusieurs histoires persistantes, et un choix de modèle par histoire, avec téléchargement à la demande unifié sur les deux plateformes.

## Portée

**Dans le périmètre :** `crates/app_desktop` uniquement (desktop ET Android — les deux plateformes partagent déjà le même code, seule la fonction de résolution de chemin diffère).

**Hors périmètre, décisions explicites :**
- **Le prototype console (`crates/app`) n'est pas touché.** Il garde `NARRATEUR_MODELE` et sa sauvegarde unique `partie.json`, exactement comme aujourd'hui — `scripts/jouer.sh` n'a besoin d'aucun changement.
- **Aucune migration** de l'actuelle sauvegarde unique `partie.json` vers le nouveau système — elle reste simplement orpheline sur disque, ignorée.
- **Pas de contenu scénarisé supplémentaire** (pas de deuxième rencontre, pas d'autre lieu) — confirmé en conversation : l'exploration/narration libre après combat suffit.
- **Pas de changement d'histoire en cours de session** sans repasser par l'écran de sélection — cet écran n'apparaît qu'au lancement de l'app, ou après la mort du personnage (voir plus bas). Changer d'histoire en pleine partie sans redémarrer n'est pas un objectif de cette itération.
- **Pas de renommage/suppression d'histoire depuis l'interface** dans cette première passe — seulement créer et reprendre. (La mort du personnage supprime toujours son histoire, comportement repris tel quel de l'actuel "une seule partie".)
- **Pas de palier de modèle ultra-léger (0,5B)** — écarté en conversation pour risque de fiabilité trop faible.

## Modèles proposés

Vérifiés par une vraie requête HTTP (pas estimés) aujourd'hui, même éditeur de quantisation déjà utilisé et éprouvé (`bartowski`) :

| Palier | Modèle | Fichier | Taille réelle |
| --- | --- | --- | --- |
| Léger | Qwen2.5-1.5B-Instruct | `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` | 986 048 768 octets (≈ 0,99 Go) |
| Équilibré (défaut actuel) | Qwen2.5-3B-Instruct | `Qwen2.5-3B-Instruct-Q4_K_M.gguf` | 1 929 903 264 octets (≈ 1,93 Go) |
| Costaud | Qwen2.5-7B-Instruct | `Qwen2.5-7B-Instruct-Q4_K_M.gguf` | 4 683 074 240 octets (≈ 4,68 Go) |

Tous trois sont de la même famille Qwen2.5-Instruct : `llama_narrateur.rs` lit déjà le gabarit de discussion directement depuis les métadonnées du GGUF (`model.chat_template(None)` + `apply_chat_template`, voir le commentaire existant dans ce fichier), donc aucun changement de code n'est nécessaire pour qu'un modèle d'une taille différente de cette même famille fonctionne. Le palier « Costaud » doit être présenté avec un avertissement dans l'interface (plus lent et plus gourmand en mémoire, surtout sur mobile) mais reste disponible sur les deux plateformes — l'utilisateur a explicitement demandé la parité desktop/Android plutôt qu'un palier réservé au desktop.

Les modèles téléchargés sont stockés dans **un répertoire partagé**, pas un par histoire : deux histoires utilisant le même palier réutilisent le même fichier déjà téléchargé.

## Stockage : histoires multiples

Remplace le fichier unique `partie.json` par :

- `sauvegardes/index.json` — un tableau d'entrées légères, une par histoire :
  ```json
  { "id": "elowen-4f21a9", "nom_personnage": "Elowen", "classe": "Guerrier", "palette_modele": "equilibre", "cree_le": "2026-11-08T10:00:00Z" }
  ```
  C'est la seule source consultée pour l'écran de sélection (un seul fichier à lire, pas besoin de désérialiser chaque `EtatPartie` juste pour afficher la liste).
- `sauvegardes/<id>.json` — l'état de jeu complet (`EtatPartie`, format JSON inchangé), toujours via `SauvegardeFichier` existant (déjà paramétré par un `PathBuf` par instance — aucun changement nécessaire dans `application`/`infra` pour ça).
- `id` : nom du personnage normalisé (minuscules, espaces retirés) + un court suffixe (nanosecondes depuis l'époque, tronquées) pour l'unicité — purement interne, jamais affiché.
- Répertoire de base : même convention que l'actuelle sauvegarde (relatif au `cwd` sur desktop, répertoire privé de l'app sur Android via `repertoire_donnees_app()` déjà existant).

`domain`/`application` restent **entièrement inchangés** par ce projet : `EtatPartie` ne gagne pas de champ `id`/`palette_modele` — ces deux informations sont gérées uniquement par `app_desktop`, dans l'index. Ça garde le changement contenu à une seule couche.

## Séquence de démarrage (le vrai changement structurel)

Aujourd'hui, `worker::lancer()` charge **un** modèle fixe avant même de savoir quelle histoire sera jouée. Avec un modèle par histoire, l'ordre s'inverse : choisir l'histoire d'abord, charger son modèle ensuite.

1. Au lancement, le thread de jeu lit `sauvegardes/index.json` (liste vide si absent — premier lancement) et envoie la liste à l'interface. **Aucun modèle n'est chargé à ce stade.**
2. Nouvel écran `Vue::SelectionHistoire` : liste les histoires existantes (nom, classe, palier de modèle) + un bouton « Nouvelle histoire ».
3. **Reprendre une histoire existante** : l'interface envoie l'id choisi → le worker résout le chemin du modèle de cette histoire dans le répertoire partagé, lance le flux de téléchargement existant si absent (déjà construit : sonde de taille, modale de confirmation, barre de progression — juste généralisé pour accepter n'importe quel palier au lieu d'une URL unique en dur), charge le modèle, charge `<id>.json`, passe en `Vue::Jeu`.
4. **Nouvelle histoire** : l'écran de création existant, avec en plus un sélecteur de palier de modèle. À la validation : génère un nouvel id, crée le personnage et le premier combat (comme aujourd'hui), résout/télécharge le modèle choisi, l'enregistre dans l'index, sauvegarde `<id>.json`, passe en `Vue::Jeu`.
5. **Mort du personnage** : comme aujourd'hui son histoire est supprimée (entrée d'index + fichier), mais au lieu de revenir directement à la création, l'app revient à `Vue::SelectionHistoire` (puisqu'il peut maintenant exister d'autres histoires à reprendre).

Conséquence à vérifier à l'implémentation (pas une inconnue bloquante, un point à confirmer empiriquement comme les précédentes incertitudes Dioxus/llama.cpp de ce projet) : le worker doit pouvoir charger un modèle, terminer une histoire, puis en charger un **autre** (palier différent) dans le même processus — pas de changement à chaud en cours de partie, juste un chargement séquentiel. `LlamaNarrateur` doit pouvoir être abandonné (`Drop`) puis reconstruit normalement ; rien dans son code actuel ne laisse penser le contraire, mais ça n'a jamais été exercé deux fois dans le même processus jusqu'ici.

## Fichiers

Nouveaux (`crates/app_desktop/src/`) :
- `histoires.rs` — `InfoHistoire` (id, nom, classe, palette, date), lecture/écriture de `index.json`, génération d'id, suppression d'une entrée.
- `modeles.rs` — `PaletteModele { Leger, Equilibre, Costaud }` avec nom affiché, URL, nom de fichier, taille approximative, et le texte d'avertissement pour Costaud.
- `vues/selection_histoire.rs` — nouvel écran.

Modifiés :
- `telechargement.rs` — `verifier_taille_modele`/`telecharger_modele` prennent un `&PaletteModele` (ou son URL/nom de fichier) au lieu des constantes `MODELE_URL` en dur.
- `protocole.rs` — `MessageVersUi::ListeHistoires(Vec<InfoHistoire>)` ; `ConfirmationTelechargementRequise`/`ProgressionTelechargement` gagnent de quoi identifier quel palier est en cours (pour l'affichage) ; `MessageVersWorker::ReprendreHistoire(String)` ; `CreerPersonnage` gagne `palette_modele` ; `Vue::SelectionHistoire(Vec<InfoHistoire>)`.
- `worker.rs` — restructuration de `lancer()` décrite ci-dessus ; `chemin_modele_par_defaut`/`chemin_sauvegarde_par_defaut` (un fichier fixe chacun) deviennent des fonctions de **répertoire de base** (un répertoire de sauvegardes, un répertoire de modèles partagés), desquelles `histoires.rs`/`modeles.rs` dérivent les chemins réels.
- `main.rs` — câblage des nouveaux messages/vue ; le bouton « Nouvelle partie » de l'écran de fin renvoie vers `Vue::SelectionHistoire` au lieu de `Vue::Creation`.
- `vues/creation.rs` — ajout du sélecteur de palier.
- `vues/mod.rs`, `styles.rs` — déclaration du nouveau module, style de l'écran de sélection (réutilise `.carte`/`.bouton-principal` déjà existants autant que possible).

Non touchés : `domain`, `application`, `crates/app` (console).

## Suppression de `NARRATEUR_MODELE` (desktop, `app_desktop` uniquement)

`worker::chemin_modele_par_defaut()` perd sa variante desktop actuelle (lecture de `NARRATEUR_MODELE`, sortie en erreur si absente) — remplacée par la résolution par palier décrite plus haut, commune aux deux plateformes. Le prototype console garde `NARRATEUR_MODELE` séparément (fichier non touché).

## Ce qui reste au charge du développeur

- Mise en forme exacte de l'écran de sélection et du sélecteur de palier (dans le thème sombre/doré déjà établi) — pas de maquette imposée.
- Texte exact de l'avertissement sur le palier Costaud.
- Détails d'implémentation de la génération d'id (le principe — nom normalisé + suffixe temporel — est fixé, la fonction exacte ne l'est pas).
- Vérifier empiriquement le chargement séquentiel de deux modèles différents dans le même processus (voir plus haut) ; adapter si le comportement réel diffère de ce qui est attendu ici, même principe que toutes les précédentes incertitudes Dioxus/llama.cpp de ce projet — le compilateur et le comportement réel font autorité.
