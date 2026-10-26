use dioxus::prelude::*;

use crate::protocole::MessageVersWorker;
use crate::telechargement::formater_taille_go;

#[component]
pub fn ConfirmationTelechargement(
    taille_octets: u64,
    sur_reponse: EventHandler<MessageVersWorker>,
) -> Element {
    let taille = formater_taille_go(taille_octets);
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Télécharger le modèle" }
                p {
                    "Premier lancement : le modèle de narration ({taille}) doit être "
                    "téléchargé avant de pouvoir jouer. Une connexion stable et "
                    "l'espace de stockage correspondant sont nécessaires."
                }
                div { class: "actions-modale",
                    button {
                        class: "bouton-principal",
                        onclick: move |_| sur_reponse.call(MessageVersWorker::ConfirmerTelechargement(true)),
                        "Télécharger"
                    }
                    button {
                        class: "bouton-secondaire",
                        onclick: move |_| sur_reponse.call(MessageVersWorker::ConfirmerTelechargement(false)),
                        "Annuler"
                    }
                }
            }
        }
    }
}
