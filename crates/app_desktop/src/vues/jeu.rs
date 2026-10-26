use dioxus::prelude::*;
use domain::EtatPartie;

use crate::protocole::MessageVersWorker;

#[component]
pub fn Jeu(
    etat: EtatPartie,
    journal: Vec<String>,
    narration_en_cours: String,
    tour_en_cours: bool,
    sur_action: EventHandler<MessageVersWorker>,
) -> Element {
    let mut saisie = use_signal(String::new);
    let mut pnj_deplie: Signal<Option<String>> = use_signal(|| None);

    let mut envoyer = move || {
        if !saisie().trim().is_empty() && !tour_en_cours {
            sur_action.call(MessageVersWorker::JouerTour(saisie()));
            saisie.set(String::new());
        }
    };

    rsx! {
        div {
            header {
                span { "{etat.lieu_actuel}" }
                span { " — PV : {etat.personnage.pv_actuels}/{etat.personnage.pv_max}" }
            }

            if let Some(combat) = etat.combat_en_cours.as_ref() {
                if !combat.termine {
                    div {
                        h3 { "Combat" }
                        for ennemi in combat.ennemis.iter().filter(|e| !e.est_mort()) {
                            div {
                                key: "{ennemi.nom}",
                                "{ennemi.nom} : {ennemi.pv_actuels}/{ennemi.pv_max} PV"
                            }
                        }
                    }
                }
            }

            div {
                for ligne in journal.iter() {
                    p { "{ligne}" }
                }
                if !narration_en_cours.is_empty() {
                    p { "{narration_en_cours}" }
                }
            }

            div {
                input {
                    value: "{saisie}",
                    disabled: tour_en_cours,
                    oninput: move |evt| saisie.set(evt.value()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            envoyer();
                        }
                    },
                }
                button {
                    disabled: tour_en_cours,
                    onclick: move |_| envoyer(),
                    "Envoyer"
                }
            }

            if !etat.pnj_connus.is_empty() {
                aside {
                    h3 { "PNJ rencontrés" }
                    for pnj in etat.pnj_connus.iter() {
                        div {
                            key: "{pnj.nom}",
                            onclick: {
                                let nom = pnj.nom.clone();
                                move |_| {
                                    let actuel = pnj_deplie();
                                    pnj_deplie.set(if actuel.as_deref() == Some(nom.as_str()) { None } else { Some(nom.clone()) });
                                }
                            },
                            strong { "{pnj.nom}" }
                            span { " — {pnj.description}" }
                            if pnj_deplie().as_deref() == Some(pnj.nom.as_str()) {
                                ul {
                                    for entree in pnj.historique.iter() {
                                        li { "{entree}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
