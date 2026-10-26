mod protocole;
mod vues;
mod worker;

use dioxus::prelude::*;
use futures_util::StreamExt;
use protocole::{MessageVersUi, MessageVersWorker, Vue};
use vues::creation::Creation;
use vues::erreur_chargement::ErreurChargement;
use vues::fin_de_partie::FinDePartie;
use vues::jeu::Jeu;

type RxPartage =
    std::rc::Rc<std::cell::RefCell<Option<futures_channel::mpsc::UnboundedReceiver<MessageVersUi>>>>;

// `dioxus::LaunchBuilder::with_context` requires its value to be
// `Any + Clone + Send + Sync + 'static` (its internal `ContextFn` is a
// `Send + Sync` closure). `RxPartage` (an `Rc<RefCell<_>>>` —
// deliberately single-threaded) is neither `Send` nor `Sync` at all, so
// it cannot go through `.with_context(...)`. `std::sync::mpsc::Sender`
// IS `Sync` (since Rust 1.72, for `T: Send`, which `MessageVersWorker`
// is), so it could in principle go through `.with_context` on its own —
// but we route both values through the same thread-local + `use_hook`
// mechanism below for consistency, rather than splitting them across two
// different sharing strategies for no real benefit: stash both values in
// thread-local cells before `.launch(...)` and pull them out exactly
// once inside `App`'s first render via `use_hook` (which only requires
// `Clone + 'static`). This is sound because `App`'s first render runs
// synchronously on the same thread `main` calls `.launch()` from.
thread_local! {
    static TX_INITIAL: std::cell::RefCell<Option<std::sync::mpsc::Sender<MessageVersWorker>>> =
        std::cell::RefCell::new(None);
    static RX_PARTAGE_INITIAL: std::cell::RefCell<Option<RxPartage>> = std::cell::RefCell::new(None);
}

fn main() {
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        std::process::exit(1);
    });
    let chemin_sauvegarde = std::path::PathBuf::from("partie.json");

    let (tx_vers_worker, rx_vers_worker) = std::sync::mpsc::channel::<MessageVersWorker>();
    let (tx_vers_ui, rx_vers_ui) = futures_channel::mpsc::unbounded::<MessageVersUi>();

    std::thread::spawn({
        let chemin_modele = std::path::PathBuf::from(chemin_modele);
        let chemin_sauvegarde = chemin_sauvegarde.clone();
        move || worker::lancer(rx_vers_worker, tx_vers_ui, chemin_modele, chemin_sauvegarde)
    });

    // `UnboundedReceiver<MessageVersUi>` has a single owner and is not
    // `Clone` — it can't be handed to the root component as an ordinary
    // value the way `tx_vers_worker` (whose `Sender` type IS `Clone`)
    // can. `Rc<RefCell<Option<_>>>` is the standard one-time handoff for
    // this: cloning the `Rc` is cheap and shares the same cell, and
    // `.take()` inside `App`'s `use_future` (which Dioxus only ever runs
    // once, on first render) extracts the real receiver exactly once.
    // `Rc`/`RefCell` (not `Arc`/`Mutex`) are correct here — this value
    // never leaves the Dioxus UI thread.
    let rx_partage: RxPartage = std::rc::Rc::new(std::cell::RefCell::new(Some(rx_vers_ui)));

    TX_INITIAL.with(|cell| *cell.borrow_mut() = Some(tx_vers_worker));
    RX_PARTAGE_INITIAL.with(|cell| *cell.borrow_mut() = Some(rx_partage));

    dioxus::LaunchBuilder::desktop().launch(App);
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
                }
                MessageVersUi::TourTermine { etat: nouvel_etat, resultat_narration, combat_termine, .. } => {
                    journal.write().push(resultat_narration);
                    narration_en_cours.set(String::new());
                    tour_en_cours.set(false);
                    let vainqueur = nouvel_etat.combat_en_cours.as_ref().and_then(|c| c.vainqueur);
                    etat.set(Some(nouvel_etat));
                    if combat_termine {
                        vue.set(Vue::FinDePartie(vainqueur));
                    }
                }
                MessageVersUi::TourEchoue(message) => {
                    if erreur_chargement_initiale() {
                        vue.set(Vue::ErreurChargement(message));
                    } else {
                        journal.write().push(format!("⚠ {}", message));
                        tour_en_cours.set(false);
                    }
                }
                MessageVersUi::PartieReinitialisee => {
                    etat.set(None);
                    journal.set(Vec::new());
                    vue.set(Vue::Creation);
                }
            }
        }
        }
    });

    match vue() {
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
                        let _ = tx.send(msg);
                    },
                }
            }
        }
        Vue::FinDePartie(vainqueur) => rsx! {
            FinDePartie {
                vainqueur,
                sur_nouvelle_partie: move |_| { let _ = tx.send(MessageVersWorker::NouvellePartie); }
            }
        },
    }
}
