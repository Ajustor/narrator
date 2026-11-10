// Les deux features de plateforme sélectionnent des backends Dioxus
// différents (`dioxus/desktop` vs `dioxus/mobile`) et deux
// implémentations distinctes de `worker::chemin_modele_par_defaut`. En
// activer les deux, ou aucune, produit sinon des erreurs de compilation
// très éloignées de la cause réelle (fonction manquante, backend absent) :
// on signale donc l'erreur ici, à l'endroit où elle se comprend.
#[cfg(all(feature = "plateforme-desktop", feature = "plateforme-android"))]
compile_error!("les features plateforme-desktop et plateforme-android sont mutuellement exclusives");

#[cfg(not(any(feature = "plateforme-desktop", feature = "plateforme-android")))]
compile_error!("une feature de plateforme (plateforme-desktop ou plateforme-android) doit être activée");

mod histoires;
mod mise_a_jour;
mod modeles;
mod protocole;
mod styles;
mod telechargement;
mod vues;
mod worker;

use dioxus::prelude::*;
use domain::Vainqueur;
use futures_util::StreamExt;
use protocole::{EtatBanniereMaj, MessageVersUi, MessageVersWorker, Vue};
use vues::banniere_maj::BanniereMiseAJour;
use vues::confirmation_telechargement::ConfirmationTelechargement;
use vues::creation::Creation;
use vues::erreur_chargement::ErreurChargement;
use vues::fin_de_partie::FinDePartie;
use vues::jeu::Jeu;
use vues::selection_histoire::SelectionHistoire;
use vues::telechargement::Telechargement;

type RxPartage =
    std::rc::Rc<std::cell::RefCell<Option<futures_channel::mpsc::UnboundedReceiver<MessageVersUi>>>>;

/// Fait défiler le journal de narration jusqu'en bas. Appelé après tout
/// ajout de contenu (fragment de narration ou ligne de journal), via
/// `document::eval` — le signal Dioxus ne contrôle pas directement la
/// position de défilement du DOM. Le script est une chaîne littérale
/// fixe, sans aucune donnée interpolée (jamais de contenu venant du
/// joueur ou du modèle) — `document::eval` est le mécanisme documenté de
/// Dioxus pour ce besoin, pas une évaluation de code arbitraire externe.
fn faire_defiler_journal() {
    let _ = document::eval(
        "let j = document.getElementById('journal'); if (j) { j.scrollTop = j.scrollHeight; }",
    );
}

// `dioxus::LaunchBuilder::with_context` exige que sa valeur satisfasse
// `Any + Clone + Send + Sync + 'static` (sa `ContextFn` interne est une
// closure `Send + Sync`). `RxPartage` (un `Rc<RefCell<_>>>` —
// volontairement mono-thread) n'est ni `Send` ni `Sync` du tout, donc il
// ne peut pas passer par `.with_context(...)`. `std::sync::mpsc::Sender`
// EST `Sync` (depuis Rust 1.72, pour `T: Send`, ce qu'est
// `MessageVersWorker`), donc il pourrait en principe passer par
// `.with_context` seul — mais on fait transiter les deux valeurs par le
// même mécanisme thread-local + `use_hook` ci-dessous, par cohérence,
// plutôt que de les répartir sur deux stratégies de partage différentes
// sans bénéfice réel : on range les deux valeurs dans des cellules
// thread-local avant `.launch(...)`, puis on les récupère exactement une
// fois au premier rendu d'`App` via `use_hook` (qui exige seulement
// `Clone + 'static`). C'est sûr car le premier rendu d'`App` s'exécute de
// façon synchrone sur le même thread que celui qui appelle `.launch()`.
thread_local! {
    static TX_INITIAL: std::cell::RefCell<Option<std::sync::mpsc::Sender<MessageVersWorker>>> =
        std::cell::RefCell::new(None);
    static RX_PARTAGE_INITIAL: std::cell::RefCell<Option<RxPartage>> = std::cell::RefCell::new(None);
}

fn main() {
    let repertoire_sauvegardes = worker::repertoire_sauvegardes_par_defaut();
    let repertoire_modeles = worker::repertoire_modeles_par_defaut();

    let (tx_vers_worker, rx_vers_worker) = std::sync::mpsc::channel::<MessageVersWorker>();
    let (tx_vers_ui, rx_vers_ui) = futures_channel::mpsc::unbounded::<MessageVersUi>();

    std::thread::spawn({
        let repertoire_sauvegardes = repertoire_sauvegardes.clone();
        let repertoire_modeles = repertoire_modeles.clone();
        move || worker::lancer(rx_vers_worker, tx_vers_ui, repertoire_sauvegardes, repertoire_modeles)
    });

    // `UnboundedReceiver<MessageVersUi>` n'a qu'un seul propriétaire et
    // n'est pas `Clone` — on ne peut pas le confier au composant racine
    // comme une valeur ordinaire, contrairement à `tx_vers_worker` (dont
    // le type `Sender` EST `Clone`). `Rc<RefCell<Option<_>>>` est le
    // transfert à usage unique standard pour ce cas : cloner le `Rc` est
    // peu coûteux et partage la même cellule, et `.take()` dans le
    // `use_future` d'`App` (que Dioxus n'exécute qu'une seule fois, au
    // premier rendu) extrait le vrai récepteur exactement une fois.
    // `Rc`/`RefCell` (et non `Arc`/`Mutex`) sont corrects ici — cette
    // valeur ne quitte jamais le thread d'interface de Dioxus.
    let rx_partage: RxPartage = std::rc::Rc::new(std::cell::RefCell::new(Some(rx_vers_ui)));

    TX_INITIAL.with(|cell| *cell.borrow_mut() = Some(tx_vers_worker));
    RX_PARTAGE_INITIAL.with(|cell| *cell.borrow_mut() = Some(rx_partage));

    // `LaunchBuilder::desktop()` n'existe que lorsque la feature `desktop`
    // de la dépendance `dioxus` est activée (via `plateforme-desktop`) ; en
    // build Android (`plateforme-android`, qui active `dioxus/mobile` et
    // non `dioxus/desktop`), cette fonction associée n'existe pas du tout.
    // `LaunchBuilder::new()` sélectionne automatiquement la bonne
    // plateforme à partir des features cargo activées sur `dioxus`
    // (desktop vs mobile), donc ce même appel fonctionne sur les deux
    // plateformes sans `#[cfg(...)]` ici.
    dioxus::LaunchBuilder::new().launch(App);
}

#[component]
fn App() -> Element {
    let tx = use_hook(|| {
        TX_INITIAL
            .with(|cell| cell.borrow_mut().take())
            .expect("le sender ne doit être pris qu'une seule fois, à la création d'App")
    });
    let rx_partage: RxPartage = use_hook(|| {
        RX_PARTAGE_INITIAL
            .with(|cell| cell.borrow_mut().take())
            .expect("rx_partage ne doit être pris qu'une seule fois, à la création d'App")
    });
    let mut vue = use_signal(|| Vue::SelectionHistoire(Vec::new()));
    let mut etat = use_signal(|| None::<domain::EtatPartie>);
    let mut journal: Signal<Vec<String>> = use_signal(Vec::new);
    let mut narration_en_cours = use_signal(String::new);
    let mut tour_en_cours = use_signal(|| false);
    // Erreur de téléchargement/chargement survenue avant `Vue::Jeu`
    // (sélection, création, téléchargement) — récupérable : `worker::lancer`
    // revient toujours à la sélection juste après (nouveau `ListeHistoires`
    // à suivre). Distinct de `Vue::ErreurChargement`, qui reste réservée au
    // cas réellement fatal (le thread du worker est mort). Affichée par
    // `SelectionHistoire` jusqu'à ce que l'utilisateur agisse à nouveau
    // depuis cet écran.
    let mut erreur_pre_jeu: Signal<Option<String>> = use_signal(|| None);
    // Dé virtuel du joueur : `Some((faces, etiquette))` dès que le worker
    // demande un jet du joueur (voir `worker::GenerateurDeJoueur`), jusqu'à
    // ce que le tour se termine (`TourTermine`/`TourEchoue`) ou qu'un
    // second jet soit demandé dans le même tour (attaque puis dégâts).
    let mut de_en_attente: Signal<Option<(u32, String)>> = use_signal(|| None);
    // Résultat du jet en attente, affiché sur le dé dès qu'il arrive — mis
    // à `None` à chaque nouvelle demande (`DeDemande`) pour ne jamais
    // montrer le résultat d'un jet précédent sur un dé qui attend encore.
    let mut dernier_resultat_de: Signal<Option<u32>> = use_signal(|| None);
    // Bannière de mise à jour (Android seulement en pratique — voir
    // `worker::lancer`, qui n'envoie `MiseAJourDisponible` que sous
    // `plateforme-android` — mais ce signal et son rendu restent
    // inconditionnels ici : rien ne distingue cet écran des autres
    // superpositions, et `EtatBanniereMaj::Masquee` ne rend rien tant que
    // le worker n'a rien signalé). Volontairement hors de `Vue` : la
    // bannière doit rester visible au-dessus de n'importe quel écran, pas
    // remplacer celui en cours.
    let mut banniere_maj: Signal<EtatBanniereMaj> = use_signal(|| EtatBanniereMaj::Masquee);
    // `true` dès que l'utilisateur a explicitement ignoré la bannière
    // (`sur_ignorer`) — tant que c'est le cas, les ticks de progression et
    // le résultat (succès ou échec) du téléchargement EN COURS ne doivent
    // plus la réafficher : sans ce garde-fou, `ProgressionMiseAJour`/
    // `MiseAJourEchouee` (envoyés par le worker indépendamment de l'état
    // affiché côté UI) ré-écrasent `Masquee` dès le prochain pourcentage,
    // annulant silencieusement l'ignorance. Remis à `false` seulement par
    // une VRAIE nouvelle annonce (`MiseAJourDisponible`) — pas par ce même
    // téléchargement en cours.
    let mut banniere_ignoree = use_signal(|| false);
    // Garde anti-double-tap pour « Installer » : `EtatBanniereMaj` ne gagne
    // aucun état « installation en cours » (un succès ne renvoie rien côté
    // worker, voir `worker::gerer_installer_mise_a_jour` — la main passe à
    // l'installeur système), donc la bannière reste visiblement sur
    // `PreteAInstaller` après le premier tap, ce qui laisserait un second
    // tap renvoyer `InstallerMiseAJour` une deuxième fois. Remis à `false`
    // par un VRAI nouvel état (`MiseAJourDisponible` ou `MiseAJourEchouee`,
    // qui permet de retenter) — jamais par `PreteAInstaller` lui-même.
    let mut installation_demandee = use_signal(|| false);

    use_future(move || {
        let rx_partage = rx_partage.clone();
        async move {
        let mut rx = rx_partage
            .borrow_mut()
            .take()
            .expect("le récepteur ne doit être pris qu'une seule fois, par ce use_future");
        while let Some(message) = rx.next().await {
            match message {
                MessageVersUi::ListeHistoires(histoires) => {
                    vue.set(Vue::SelectionHistoire(histoires));
                }
                MessageVersUi::ValeursProposees(valeurs) => {
                    vue.set(Vue::Creation(valeurs));
                }
                MessageVersUi::PartieCreee(e) => {
                    // `PartieCreee` démarre une histoire (nouvelle ou
                    // reprise) : le journal/la narration en cours/l'état
                    // "tour en cours" appartiennent à l'histoire
                    // précédemment affichée (s'il y en avait une) et ne
                    // doivent pas survivre au changement d'histoire dans le
                    // même processus — sans ce reset, l'écran de jeu de la
                    // nouvelle histoire s'ouvrirait avec le journal entier
                    // (y compris une éventuelle narration de mort) de la
                    // précédente encore affiché au-dessus de son propre
                    // contenu.
                    //
                    // Pour une histoire REPRISE, `e.resumes` contient déjà
                    // la narration des tours précédents (en clair pour les
                    // plus récents, condensée pour les plus anciens — voir
                    // `condenser_si_necessaire` dans `application`) : on
                    // l'utilise pour réamorcer le journal plutôt que de
                    // partir d'un écran vide sans aucun historique. Pour une
                    // NOUVELLE histoire, `resumes` est vide, donc ce reste
                    // équivalent à l'ancien `Vec::new()`.
                    journal.set(e.resumes.clone());
                    narration_en_cours.set(String::new());
                    tour_en_cours.set(false);
                    etat.set(Some(e));
                    vue.set(Vue::Jeu);
                }
                MessageVersUi::NarrationFragment(fragment) => {
                    narration_en_cours.write().push_str(&fragment);
                    faire_defiler_journal();
                }
                MessageVersUi::IntroductionRacontee(texte) => {
                    if !texte.is_empty() {
                        journal.write().push(texte);
                    }
                    narration_en_cours.set(String::new());
                    faire_defiler_journal();
                }
                MessageVersUi::TourTermine { etat: nouvel_etat, resultat_narration, resume_mecanique, vainqueur_combat, partie_terminee } => {
                    de_en_attente.set(None);
                    dernier_resultat_de.set(None);
                    journal.write().push(resultat_narration);
                    for ligne in resume_mecanique {
                        journal.write().push(ligne);
                    }
                    // Une victoire ou une fuite réussie ne termine plus la
                    // partie (seule la mort le fait, voir `partie_terminee`
                    // plus bas) — on l'annonce quand même dans le journal
                    // avant de reprendre l'exploration, pour que ce ne soit
                    // pas silencieux.
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
                    // Avant Vue::Jeu (sélection, création, téléchargement,
                    // chargement du modèle), un échec est récupérable :
                    // `lancer()` revient à la sélection juste après (nouveau
                    // ListeHistoires à suivre) — on affiche l'erreur sur
                    // l'écran de sélection plutôt que sur un écran dédié qui
                    // serait immédiatement remplacé. Pendant Vue::Jeu, un
                    // échec de tour reste local au journal, comme avant.
                    if matches!(vue(), Vue::Jeu) {
                        de_en_attente.set(None);
                        dernier_resultat_de.set(None);
                        journal.write().push(format!("⚠ {}", message));
                        narration_en_cours.set(String::new());
                        tour_en_cours.set(false);
                        faire_defiler_journal();
                    } else {
                        erreur_pre_jeu.set(Some(message));
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
                MessageVersUi::DeDemande { faces, etiquette } => {
                    dernier_resultat_de.set(None);
                    de_en_attente.set(Some((faces, etiquette)));
                }
                MessageVersUi::DeResultat(valeur) => {
                    dernier_resultat_de.set(Some(valeur));
                }
                MessageVersUi::MiseAJourDisponible { version, url_apk } => {
                    // Une VRAIE nouvelle annonce : l'ignorance d'une
                    // précédente bannière (s'il y en avait une) ne tient
                    // plus face à une mise à jour différente, et un éventuel
                    // tap « Installer » d'un cycle précédent ne doit plus
                    // bloquer ce nouveau cycle.
                    banniere_ignoree.set(false);
                    installation_demandee.set(false);
                    banniere_maj.set(EtatBanniereMaj::Disponible { version, url_apk });
                }
                MessageVersUi::ProgressionMiseAJour(progression) => {
                    if !banniere_ignoree() {
                        banniere_maj.set(EtatBanniereMaj::EnTelechargement { progression });
                    }
                }
                MessageVersUi::MiseAJourTelechargee { chemin_apk } => {
                    if !banniere_ignoree() {
                        banniere_maj.set(EtatBanniereMaj::PreteAInstaller { chemin_apk });
                    }
                }
                MessageVersUi::MiseAJourEchouee(message) => {
                    // Un échec (téléchargement ou installation) doit
                    // permettre de retenter : un tap « Installer » bloqué
                    // par `installation_demandee` ne doit pas le rester
                    // pour toujours si l'installation a échoué.
                    installation_demandee.set(false);
                    if !banniere_ignoree() {
                        banniere_maj.set(EtatBanniereMaj::Echouee(message));
                    }
                }
            }
        }
        // Le canal s'est fermé : le thread de jeu s'est arrêté de façon
        // inattendue (panique ou fin de vie). Rendre cet arrêt visible
        // plutôt que de laisser la fenêtre figée sans aucune explication.
        vue.set(Vue::ErreurChargement(
            "le moteur de jeu s'est arrêté de façon inattendue — voir la sortie console pour le détail".to_string(),
        ));
        }
    });

    let tx_telecharger = tx.clone();
    let tx_installer = tx.clone();

    let vue_active = match vue() {
        Vue::ErreurChargement(message) => rsx! { ErreurChargement { message } },
        Vue::SelectionHistoire(histoires) => {
            let tx_nouvelle_histoire = tx.clone();
            rsx! {
            SelectionHistoire {
                histoires,
                erreur: erreur_pre_jeu(),
                sur_message: move |msg| {
                    erreur_pre_jeu.set(None);
                    let _ = tx.send(msg);
                },
                sur_nouvelle_histoire: move |_| {
                    erreur_pre_jeu.set(None);
                    let _ = tx_nouvelle_histoire.send(MessageVersWorker::DemarrerCreationPersonnage);
                },
            }
        }
        },
        Vue::Creation(valeurs_proposees) => rsx! {
            Creation {
                valeurs_proposees,
                sur_creation: move |msg| { let _ = tx.send(msg); }
            }
        },
        Vue::Jeu => {
            let Some(etat_actuel) = etat() else {
                return rsx! { div { "Chargement..." } };
            };
            let tx_retour_selection = tx.clone();
            let tx_de = tx.clone();
            rsx! {
                Jeu {
                    etat: etat_actuel,
                    journal: journal(),
                    narration_en_cours: narration_en_cours(),
                    tour_en_cours: tour_en_cours(),
                    de_en_attente: de_en_attente(),
                    dernier_resultat_de: dernier_resultat_de(),
                    sur_action: move |msg| {
                        tour_en_cours.set(true);
                        narration_en_cours.set(String::new());
                        let _ = tx.send(msg);
                    },
                    sur_retour_selection: move |_| {
                        let _ = tx_retour_selection.send(MessageVersWorker::RetourSelection);
                    },
                    sur_lancer_de: move |_| {
                        let _ = tx_de.send(MessageVersWorker::DeLancee);
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

    rsx! {
        document::Style { "{styles::CSS}" }
        BanniereMiseAJour {
            etat: banniere_maj(),
            sur_telecharger: move |_| {
                if let EtatBanniereMaj::Disponible { url_apk, .. } = banniere_maj() {
                    // Affiche la progression (à 0%) immédiatement, avant
                    // même la première réponse du worker : celui-ci peut
                    // être occupé (narration en cours, chargement du
                    // modèle) et mettre un moment avant d'émettre son
                    // premier `ProgressionMiseAJour`. Sans ce retour
                    // immédiat, la bannière reste sur `Disponible` assez
                    // longtemps pour qu'un second tap renvoie une deuxième
                    // fois `TelechargerMiseAJour` ; une fois l'état changé
                    // ici, ce `if let` échoue sur un second tap et l'ignore.
                    banniere_maj.set(EtatBanniereMaj::EnTelechargement { progression: 0.0 });
                    let _ = tx_telecharger.send(MessageVersWorker::TelechargerMiseAJour { url_apk });
                }
            },
            sur_installer: move |_| {
                // `EtatBanniereMaj` ne distingue pas « prête à installer »
                // de « installation déjà demandée » (un succès ne renvoie
                // rien côté worker, voir `installation_demandee` plus haut),
                // d'où ce garde-fou explicite plutôt qu'un simple `if let`
                // sur l'état de la bannière.
                if installation_demandee() {
                    return;
                }
                if let EtatBanniereMaj::PreteAInstaller { chemin_apk } = banniere_maj() {
                    installation_demandee.set(true);
                    let _ = tx_installer.send(MessageVersWorker::InstallerMiseAJour { chemin_apk });
                }
            },
            sur_ignorer: move |_| {
                banniere_ignoree.set(true);
                banniere_maj.set(EtatBanniereMaj::Masquee);
            },
        }
        {vue_active}
    }
}
