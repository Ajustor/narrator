use dioxus::prelude::*;

use crate::histoires::InfoHistoire;
use crate::protocole::MessageVersWorker;

#[component]
pub fn SelectionHistoire(
    histoires: Vec<InfoHistoire>,
    erreur: Option<String>,
    sur_message: EventHandler<MessageVersWorker>,
    sur_nouvelle_histoire: EventHandler<()>,
) -> Element {
    // Id de l'histoire pour laquelle une suppression est en attente de
    // confirmation — un deuxième clic est nécessaire avant d'envoyer
    // `SupprimerHistoire`, pour qu'un clic accidentel sur "Supprimer" ne
    // détruise pas un personnage d'un coup (pas de `confirm()` JS
    // disponible/souhaitable ici, donc confirmation en ligne à la place).
    let mut en_attente_suppression: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Tes histoires" }
                if let Some(message) = erreur {
                    p { class: "avertissement", "⚠ {message}" }
                }
                if histoires.is_empty() {
                    p { "Aucune histoire pour l'instant." }
                } else {
                    for histoire in histoires.iter() {
                        div {
                            key: "{histoire.id}",
                            class: "pnj ligne-histoire",
                            onclick: {
                                let id = histoire.id.clone();
                                move |_| sur_message.call(MessageVersWorker::ReprendreHistoire(id.clone()))
                            },
                            div {
                                strong { "{histoire.nom_personnage}" }
                                span { " — {histoire.classe} — {histoire.palette_modele.nom_affiche()}" }
                            }
                            if en_attente_suppression().as_deref() == Some(histoire.id.as_str()) {
                                div { class: "confirmation-suppression",
                                    span { "Supprimer définitivement ?" }
                                    button {
                                        class: "bouton-danger",
                                        onclick: {
                                            let id = histoire.id.clone();
                                            move |evt: Event<MouseData>| {
                                                evt.stop_propagation();
                                                en_attente_suppression.set(None);
                                                sur_message.call(MessageVersWorker::SupprimerHistoire(id.clone()));
                                            }
                                        },
                                        "Confirmer"
                                    }
                                    button {
                                        class: "bouton-secondaire",
                                        onclick: move |evt: Event<MouseData>| {
                                            evt.stop_propagation();
                                            en_attente_suppression.set(None);
                                        },
                                        "Annuler"
                                    }
                                }
                            } else {
                                button {
                                    class: "bouton-supprimer",
                                    onclick: {
                                        let id = histoire.id.clone();
                                        move |evt: Event<MouseData>| {
                                            evt.stop_propagation();
                                            en_attente_suppression.set(Some(id.clone()));
                                        }
                                    },
                                    "Supprimer"
                                }
                            }
                        }
                    }
                }
                button {
                    class: "bouton-principal",
                    onclick: move |_| sur_nouvelle_histoire.call(()),
                    "Nouvelle histoire"
                }
            }
        }
    }
}
