use dioxus::prelude::*;

use crate::histoires::InfoHistoire;
use crate::protocole::MessageVersWorker;

#[component]
pub fn SelectionHistoire(
    histoires: Vec<InfoHistoire>,
    erreur: Option<String>,
    sur_reprendre: EventHandler<MessageVersWorker>,
    sur_nouvelle_histoire: EventHandler<()>,
) -> Element {
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
                            class: "pnj",
                            onclick: {
                                let id = histoire.id.clone();
                                move |_| sur_reprendre.call(MessageVersWorker::ReprendreHistoire(id.clone()))
                            },
                            strong { "{histoire.nom_personnage}" }
                            span { " — {histoire.classe} — {histoire.palette_modele.nom_affiche()}" }
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
