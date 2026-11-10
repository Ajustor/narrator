# Mise à jour automatique (Android) depuis le réseau local

## Intention et contexte

Pendant cette session, chaque nouvel APK a été reconstruit puis hébergé via
un `python -m http.server` ad hoc sur le réseau local (actuellement
`192.168.1.48:8000`), et l'utilisateur devait revenir chercher le lien
(artefact ou QR code) pour réinstaller manuellement à chaque fois.

Demande : « ajouter un système d'update auto qui vient check le réseau
local ». Décisions confirmées pendant le brainstorming :
- La vérification est **automatique**, pas un bouton — déclenchée à
  **chaque lancement de l'app**, en arrière-plan, silencieuse si rien
  n'est trouvé (le serveur LAN ne tourne pas en permanence).
- Une fois une mise à jour détectée : une bannière prévient l'utilisateur ;
  le téléchargement ne démarre qu'après confirmation (tap).
- Après confirmation : **l'app télécharge elle-même l'APK et lance
  l'installeur Android**, plutôt que d'ouvrir le lien dans le navigateur
  (alternative plus simple et moins risquée, mais explicitement écartée —
  voir Risques ci-dessous pour ce que ce choix implique concrètement).

## Objectifs

1. À chaque lancement de l'app Android, vérifier en arrière-plan si une
   version plus récente existe sur le réseau local, sans jamais bloquer
   ni ralentir le démarrage, et sans jamais afficher d'erreur si le
   serveur n'est pas joignable (c'est l'état normal la plupart du temps).
2. Si une version plus récente est trouvée, afficher une bannière non
   intrusive proposant de la télécharger.
3. Sur confirmation, télécharger l'APK vers un emplacement accessible à
   l'installeur système, puis lancer l'écran d'installation Android
   (`PackageInstaller`), avec gestion propre du cas où l'utilisateur
   n'a pas encore accordé la permission « installer des applications
   inconnues » à cette app.

## Non-objectifs

- Pas de vraie découverte réseau (scan de sous-réseau, mDNS/Bonjour) : le
  serveur est toujours la même machine de développement sur le même
  réseau domestique ; l'adresse est une constante configurable, pas
  découverte dynamiquement. Hors scope — voir Risques.
- Pas de vérification périodique pendant que l'app est ouverte (décision
  confirmée : au lancement seulement).
- Pas de téléchargement ni d'installation automatique sans confirmation
  explicite de l'utilisateur à chaque fois (Android l'exigerait de toute
  façon pour l'étape d'installation).
- Pas de changement côté desktop : fonctionnalité entièrement derrière
  `#[cfg(feature = "plateforme-android")]`, comme le reste du code déjà
  spécifique à Android dans ce crate (`repertoire_donnees_app`).
- Pas de vrai canal de distribution signé/sécurisé type Play Store : ce
  reste un mécanisme de confort pour du sideload de développement, sur un
  serveur HTTP non authentifié — voir Anti-objectifs.

## Anti-objectifs

- Un appareil qui n'est **pas** sur le même réseau Wi-Fi que la machine de
  développement ne doit jamais planter ni afficher d'erreur visible — la
  vérification échoue silencieusement (timeout court, log seulement).
- N'importe quel autre appareil sur le même réseau Wi-Fi pourrait en
  principe répondre à la place du vrai serveur et proposer un faux
  `version.json` pointant vers un APK arbitraire. Ce risque existe déjà
  aujourd'hui (l'utilisateur télécharge et installe manuellement depuis un
  lien HTTP non chiffré) et n'est pas aggravé en nature par cette
  fonctionnalité, mais il est automatisé — donc : jamais d'installation
  sans confirmation explicite de l'utilisateur (déjà couvert par les
  objectifs), et jamais de désactivation de la demande de confirmation
  Android elle-même.

## Risques techniques identifiés (à vérifier avant de committer au plan complet)

Recherche effectuée dans le code existant (`crates/app_desktop`) :

- **Vérification + téléchargement du manifeste/APK : risque faible.** Le
  module `telechargement.rs` existe déjà (téléchargement HTTP synchrone
  via `ureq`, écriture atomique via fichier `.tmp` puis `rename`,
  réutilisé pour le modèle de langage) — directement réutilisable pour
  l'APK avec une nouvelle destination.
- **Lancer l'installeur Android : risque réel, rien d'équivalent n'existe
  encore dans ce projet.** Il faut :
  1. Déclarer un `<provider>` (`FileProvider`) + une ressource
     `res/xml/file_paths.xml` + la permission
     `android.permission.REQUEST_INSTALL_PACKAGES` dans le manifest
     Android généré. `dx` 0.7 expose des sections `[permissions]` et
     `[android.raw.manifest]` dans `Dioxus.toml` à cet effet, mais
     **aucune des deux n'a jamais été utilisée ni testée dans ce
     projet** — seul `scripts/patch_icone_android.sh` patch le projet
     Android généré après coup, et ce script note explicitement que
     `dx build`/`dx bundle` écrase tout patch à chaque invocation (donc
     un futur patch manuel du manifest devrait suivre le même motif
     qu'un script réappliqué après chaque build, à moins que les
     sections `Dioxus.toml` fonctionnent réellement).
  2. Appeler `FileProvider.getUriForFile(...)` via JNI pour obtenir
     l'URI `content://` à passer à l'installeur — un appel JNI plus
     complexe que celui qui existe aujourd'hui (`Activity.getFilesDir()`) :
     `FileProvider` n'est pas chargée par le classloader natif par
     défaut, il faut probablement remonter par le classloader de
     l'activité pour la résoudre.
  3. Construire un `Intent` (`ACTION_VIEW`, type MIME
     `application/vnd.android.package-archive`,
     `FLAG_GRANT_READ_URI_PERMISSION`) et appeler
     `Activity.startActivity(...)` — également un nouvel appel JNI,
     bien que du même type que celui qui existe déjà.
  4. **Permission utilisateur distincte, mais gérée par le système, pas
     par nous** : Android exige que l'utilisateur accorde « Installer des
     applications inconnues » à Narrateur spécifiquement (différente de
     celle déjà accordée à son navigateur) — mais c'est l'écran du
     `PackageInstaller` système lui-même qui affiche son propre message et
     son propre bouton vers les réglages quand la permission manque,
     automatiquement, sans action de notre part (comportement standard
     depuis Android 8). Voir la résolution détaillée plus bas : on se
     contente de capturer `ActivityNotFoundException` côté JNI, rien de
     plus.
- **Network Security Config (NSC) : risque vérifié, résolu par raisonnement
  technique, pas encore par un test réel.** Le `network_security_config.xml`
  généré par `dx` (vérifié directement :
  `target/dx/Narrator/release/android/app/app/src/main/res/xml/network_security_config.xml`)
  n'autorise le trafic en clair (HTTP non chiffré) QUE vers `127.0.0.1` —
  `targetSdk = 34` fait qu'Android bloque par défaut le HTTP en clair vers
  tout autre hôte. Le serveur LAN (`192.168.1.48:8000`) n'est pas en HTTPS,
  donc cette politique semble menacer tout le mécanisme.
  **Résolution retenue** : cette politique (NSC) est appliquée par la
  pile réseau Java d'Android (`HttpURLConnection`/OkHttp/WebView), pas au
  niveau des sockets natifs. `ureq` (comme `telechargement.rs` l'utilise
  déjà aujourd'hui pour télécharger le modèle de langage en HTTPS depuis
  huggingface.co) passe par `std::net`/les sockets POSIX du NDK
  directement, sans jamais traverser la couche Java où la NSC est
  appliquée — ce téléchargement HTTPS fonctionnant déjà de façon fiable
  sur un vrai appareil est un indice fort (bien que spécifique à HTTPS,
  pas au cas clair) que les requêtes de ce projet via `ureq` échappent
  structurellement à l'enforcement NSC. Sur cette base, je pars du
  principe que le HTTP en clair vers `192.168.1.48` fonctionnera aussi
  via `ureq`, sans modification du manifest.
  **Filet de sécurité si ce raisonnement se révèle faux** (constaté comme
  un échec silencieux de connexion lors d'un vrai test sur appareil,
  alors que le serveur LAN répond bien) : ajouter une entrée
  `<domain-config>` pour l'IP du serveur dans
  `network_security_config.xml`, via le même motif de script de patch
  post-`dx-build` que `scripts/patch_icone_android.sh` (ce fichier n'est
  piloté par aucune section connue de `Dioxus.toml` à ce jour).
- **`minSdk = 24` précède le modèle de permission par-app (API 26+).**
  Sur Android 7.0/7.1 (API 24-25), le réglage est un unique interrupteur
  global « Sources inconnues », pas une permission par application — le
  comportement exact de l'écran d'installation y diffère légèrement.
  Risque accepté tel quel (l'appareil réel de test est presque
  certainement bien plus récent) ; pas de changement de `minSdk` pour
  cette fonctionnalité, qui serait une décision plus large et sans lien
  direct avec elle.
- **Pas de vérification sur appareil réel possible depuis cette session**
  : aucun appareil Android n'est connecté (`adb devices` vide), et aucun
  émulateur n'est disponible/configuré. Toute cette fonctionnalité ne
  pourra être validée que par relecture de code + compilation jusqu'à ce
  que l'utilisateur installe et teste sur son propre téléphone — écart de
  vérification à assumer explicitement, cohérent avec l'attente du projet
  de vérification empirique réelle (ici impossible avant un vrai test
  utilisateur).

## Décisions techniques (miennes, marquées comme telles)

- **Format du manifeste de version** : un petit fichier JSON,
  `version.json`, servi sur le même serveur LAN que l'APK (même
  répertoire que `narrateur.apk`), avec la forme :
  ```json
  { "version": "<hash court du commit git>", "url": "http://<ip>:<port>/narrateur.apk" }
  ```
  Je régénère ce fichier moi-même à chaque reconstruction/hébergement
  d'un nouvel APK (déjà une étape manuelle que je fais systématiquement),
  en y mettant le hash court du commit (`git rev-parse --short HEAD`).
- **Identité de version côté app** : plutôt que de dépendre d'un
  `versionCode`/`versionName` Android jamais incrémenté manuellement
  jusqu'ici (`Dioxus.toml`/gradle, toujours `1`/`0.1.0` dans ce projet),
  le hash court du commit git au moment du build est injecté dans le
  binaire via un `build.rs` (nouveau, dans `crates/app_desktop/`) qui
  exécute `git rev-parse --short HEAD` et l'expose via
  `env!("NARRATEUR_GIT_HASH")`. Comparaison simple de chaînes avec le
  champ `version` du manifeste téléchargé : différent → mise à jour
  disponible. Si `build.rs` échoue à lire le hash (pas un dépôt git,
  `git` absent...), se replier sur la chaîne `"inconnu"` — dans ce cas
  toute vérification serait positive (hash du manifeste toujours
  différent de `"inconnu"`), ce qui est acceptable : ce mécanisme ne
  tourne que sur des builds de développement faits depuis ce dépôt.
- **Adresse du serveur à vérifier** : une constante Rust
  (`ADRESSE_MAJ_PAR_DEFAUT: &str = "192.168.1.48:8000"`), pas de
  découverte réseau (hors scope). Documentée en commentaire comme
  nécessitant une mise à jour manuelle si l'adresse IP de la machine de
  développement change un jour.
- **Nouveau module** `crates/app_desktop/src/mise_a_jour.rs` :
  - `verifier_disponibilite(adresse: &str) -> Option<InfoMiseAJour>` —
    requête HTTP GET bloquante vers `http://{adresse}/version.json` via
    `ureq` (même bibliothèque que `telechargement.rs`), délai court
    (ex. 3s connect/read — plus court que les 30s du téléchargement du
    modèle, puisque c'est une vérification silencieuse censée échouer
    vite la plupart du temps), parse JSON minimal (pas de dépendance
    `serde_json` supplémentaire nécessaire si une extraction de chaîne
    suffit, sinon réutiliser `serde_json` déjà présent dans le
    workspace). Toute erreur (connexion, timeout, JSON invalide) →
    `None`, jamais de panique ni de message affiché.
  - `InfoMiseAJour { version: String, url_apk: String }`.
  - `telecharger_apk(url: &str, destination: &Path, sur_progres: &mut dyn FnMut(u8)) -> Result<(), ErreurTelechargement>`
    — réutilise directement le motif de `telechargement.rs`
    (fichier `.tmp` puis `rename` atomique).
  - `#[cfg(feature = "plateforme-android")] fn lancer_installation(etat: &EtatInstallationAndroid /* ou équivalent */, chemin_apk: &Path) -> Result<(), ErreurInstallation>`
    — le nouveau JNI : résout l'URI `content://` via `FileProvider`,
    construit l'`Intent`, appelle `startActivity`. Si la permission
    « installer des apps inconnues » n'est pas accordée, Android lance
    normalement l'installeur quand même et affiche SA PROPRE invite de
    permission à ce moment-là (comportement standard du système) — donc
    pas besoin de vérifier la permission nous-mêmes au préalable ; il
    faut seulement s'assurer que notre appel ne plante pas si l'Intent
    ne peut pas être résolu (`ActivityNotFoundException` côté Java,
    à transformer en erreur Rust propre plutôt qu'un crash JNI).
- **Déclenchement** : au lancement de l'app Android, après l'écran de
  sélection d'histoire mais sans bloquer son affichage — un
  `use_future`/message worker asynchrone de plus, suivant le même motif
  que les vérifications existantes au démarrage (téléchargement du
  modèle, etc.), qui envoie un nouveau variant de `MessageVersUi` si une
  mise à jour est trouvée.
- **UI** : une bannière fixée en haut de l'écran courant (pas un nouvel
  écran dédié), visible quelle que soit la vue active (sélection
  d'histoire, création, jeu), avec le texte « Mise à jour disponible » et
  un bouton « Télécharger ». Pendant le téléchargement, affiche une barre
  de progression (réutilise le motif déjà existant pour le téléchargement
  du modèle de langage). Une fois téléchargé, le bouton devient
  « Installer » (tap → `lancer_installation`). La bannière reste
  dismissible (un petit bouton « plus tard ») sans bloquer l'usage normal
  de l'app.
- **Manifest Android** : tentative via `Dioxus.toml`
  (`[permissions]`/`[android.raw.manifest]`) en premier ; si cette voie
  ne fonctionne pas empiriquement (à vérifier tôt, voir Risques), repli
  sur un script de patch post-`dx-build` suivant exactement le même motif
  que `scripts/patch_icone_android.sh` (réappliqué après chaque
  `dx build`/`dx bundle`, avant `gradlew assembleRelease`).

## Ce qui reste au développeur

- Formulation exacte du texte de la bannière et des messages d'erreur
  (ex. si `ActivityNotFoundException`).
- Emplacement exact de l'icône/texte de la bannière dans la hiérarchie
  RSX existante (quel composant parent la rend).
- Nom exact de la permission/section dans `Dioxus.toml` à utiliser en
  premier (`[permissions]` vs `[android.raw.manifest]`) — à déterminer
  empiriquement lors de l'implémentation, voir Risques.
