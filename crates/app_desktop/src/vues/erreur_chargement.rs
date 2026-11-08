use dioxus::prelude::*;

/// Conseil affiché sous le message d'erreur. Cet écran n'est plus affiché
/// que lorsque le thread worker est mort de façon fatale et irrécupérable :
/// tout échec récupérable (téléchargement, chargement du modèle) passe
/// désormais par un autre mécanisme et retourne à l'écran de sélection des
/// histoires plutôt que d'arriver ici. Le conseil dépend tout de même de
/// la plateforme : desktop n'a plus de variable d'environnement
/// `NARRATEUR_MODELE` (les modèles sont téléchargés à la demande depuis le
/// catalogue, comme sur Android), donc il n'y a rien de spécifique à
/// conseiller au-delà de relancer l'application.
#[cfg(feature = "plateforme-desktop")]
const CONSEIL: &str = "Le moteur de jeu s'est arrêté de façon inattendue. Relance l'application.";

#[cfg(feature = "plateforme-android")]
const CONSEIL: &str = "Vérifie ta connexion réseau et l'espace de stockage disponible, puis relance l'application.";

#[component]
pub fn ErreurChargement(message: String) -> Element {
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Échec du démarrage du narrateur" }
                p { "{message}" }
                p { "{CONSEIL}" }
            }
        }
    }
}
