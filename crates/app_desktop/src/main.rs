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

mod modeles;
mod protocole;
mod styles;
mod telechargement;
mod vues;
mod worker;

use dioxus::prelude::*;
use domain::Vainqueur;
use futures_util::StreamExt;
use protocole::{MessageVersUi, MessageVersWorker, Vue};
use vues::confirmation_telechargement::ConfirmationTelechargement;
use vues::creation::Creation;
use vues::erreur_chargement::ErreurChargement;
use vues::fin_de_partie::FinDePartie;
use vues::jeu::Jeu;
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
    let chemin_modele = worker::chemin_modele_par_defaut();
    let chemin_sauvegarde = worker::chemin_sauvegarde_par_defaut();

    let (tx_vers_worker, rx_vers_worker) = std::sync::mpsc::channel::<MessageVersWorker>();
    let (tx_vers_ui, rx_vers_ui) = futures_channel::mpsc::unbounded::<MessageVersUi>();

    std::thread::spawn({
        let chemin_modele = chemin_modele.clone();
        let chemin_sauvegarde = chemin_sauvegarde.clone();
        move || worker::lancer(rx_vers_worker, tx_vers_ui, chemin_modele, chemin_sauvegarde)
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
    let mut vue = use_signal(|| Vue::Creation);
    let mut etat = use_signal(|| None::<domain::EtatPartie>);
    let mut journal: Signal<Vec<String>> = use_signal(Vec::new);
    let mut narration_en_cours = use_signal(String::new);
    let mut tour_en_cours = use_signal(|| false);
    let mut erreur_chargement_initiale = use_signal(|| true);

    use_future(move || {
        let rx_partage = rx_partage.clone();
        async move {
        let mut rx = rx_partage
            .borrow_mut()
            .take()
            .expect("le récepteur ne doit être pris qu'une seule fois, par ce use_future");
        while let Some(message) = rx.next().await {
            match message {
                MessageVersUi::EtatInitial(Some(e)) => {
                    etat.set(Some(e));
                    vue.set(Vue::Jeu);
                    erreur_chargement_initiale.set(false);
                }
                MessageVersUi::EtatInitial(None) => {
                    vue.set(Vue::Creation);
                    erreur_chargement_initiale.set(false);
                }
                MessageVersUi::PartieCreee(e) => {
                    etat.set(Some(e));
                    vue.set(Vue::Jeu);
                }
                MessageVersUi::NarrationFragment(fragment) => {
                    narration_en_cours.write().push_str(&fragment);
                    faire_defiler_journal();
                }
                MessageVersUi::TourTermine { etat: nouvel_etat, resultat_narration, resume_mecanique, vainqueur_combat, partie_terminee } => {
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
                    if erreur_chargement_initiale() {
                        vue.set(Vue::ErreurChargement(message));
                    } else {
                        journal.write().push(format!("⚠ {}", message));
                        narration_en_cours.set(String::new());
                        tour_en_cours.set(false);
                        faire_defiler_journal();
                    }
                }
                MessageVersUi::ProgressionTelechargement(progression) => {
                    vue.set(Vue::Telechargement(progression));
                }
                MessageVersUi::ConfirmationTelechargementRequise(taille_octets) => {
                    vue.set(Vue::ConfirmationTelechargement(taille_octets));
                }
                MessageVersUi::PartieReinitialisee => {
                    etat.set(None);
                    journal.set(Vec::new());
                    vue.set(Vue::Creation);
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

    let vue_active = match vue() {
        Vue::ErreurChargement(message) => rsx! { ErreurChargement { message } },
        Vue::Creation => rsx! {
            Creation {
                sur_creation: move |msg| { let _ = tx.send(msg); }
            }
        },
        Vue::Jeu => {
            let Some(etat_actuel) = etat() else {
                return rsx! { div { "Chargement..." } };
            };
            rsx! {
                Jeu {
                    etat: etat_actuel,
                    journal: journal(),
                    narration_en_cours: narration_en_cours(),
                    tour_en_cours: tour_en_cours(),
                    sur_action: move |msg| {
                        tour_en_cours.set(true);
                        narration_en_cours.set(String::new());
                        let _ = tx.send(msg);
                    },
                }
            }
        }
        Vue::Telechargement(progression) => rsx! { Telechargement { progression } },
        Vue::ConfirmationTelechargement(taille_octets) => rsx! {
            ConfirmationTelechargement {
                taille_octets,
                sur_reponse: move |msg| { let _ = tx.send(msg); }
            }
        },
        Vue::FinDePartie(vainqueur) => rsx! {
            FinDePartie {
                vainqueur,
                sur_nouvelle_partie: move |_| { let _ = tx.send(MessageVersWorker::NouvellePartie); }
            }
        },
    };

    rsx! {
        document::Style { "{styles::CSS}" }
        {vue_active}
    }
}
