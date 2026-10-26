use dioxus::prelude::*;
use domain::Vainqueur;

#[component]
pub fn FinDePartie(vainqueur: Option<Vainqueur>, sur_nouvelle_partie: EventHandler<()>) -> Element {
    let message = match vainqueur {
        Some(Vainqueur::Joueur) => "Victoire ! Le gobelin est vaincu.",
        Some(Vainqueur::Ennemis) => "Tu es tombé au combat...",
        Some(Vainqueur::Fuite) => "Tu as fui le combat.",
        None => "La partie est terminée.",
    };

    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "{message}" }
                button {
                    class: "bouton-principal",
                    onclick: move |_| sur_nouvelle_partie.call(()),
                    "Nouvelle partie"
                }
            }
        }
    }
}
