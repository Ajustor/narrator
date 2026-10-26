use dioxus::prelude::*;

/// Conseil affiché sous le message d'erreur. Cet écran ne sert plus
/// seulement à l'échec de *chargement* du modèle : il affiche aussi les
/// échecs de *téléchargement* (Android, premier lancement) et l'arrêt
/// inattendu du moteur. Le conseil dépend donc de la plateforme plutôt
/// que de supposer un contexte desktop : sur Android, `NARRATEUR_MODELE`
/// n'existe pas du tout, et conseiller d'y pointer un `.gguf` n'aurait
/// aucun sens pour un joueur dont le téléchargement a échoué.
#[cfg(feature = "plateforme-desktop")]
const CONSEIL: &str = "Vérifie ta connexion réseau, l'espace de stockage disponible, et que la variable d'environnement NARRATEUR_MODELE pointe vers un fichier .gguf valide, puis relance l'application.";

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
