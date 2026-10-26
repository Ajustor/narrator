use dioxus::prelude::*;

#[allow(dead_code)]
#[component]
pub fn ErreurChargement(message: String) -> Element {
    rsx! {
        div {
            h1 { "Échec du chargement du modèle" }
            p { "{message}" }
            p { "Vérifie que la variable d'environnement NARRATEUR_MODELE pointe vers un fichier .gguf valide, puis relance l'application." }
        }
    }
}
