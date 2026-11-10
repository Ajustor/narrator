use dioxus::prelude::*;

use crate::protocole::EtatBanniereMaj;

/// Bannière de mise à jour, affichée par `main.rs` au-dessus de
/// `vue_active` quel que soit l'écran actif — purement présentationnelle :
/// tout l'état (`EtatBanniereMaj`) et l'envoi des messages worker restent
/// dans `main.rs`/`worker.rs`, pas ici.
#[component]
pub fn BanniereMiseAJour(
    etat: EtatBanniereMaj,
    sur_telecharger: EventHandler<()>,
    sur_installer: EventHandler<()>,
    sur_ignorer: EventHandler<()>,
) -> Element {
    let disponible = match &etat {
        EtatBanniereMaj::Disponible { version, .. } => Some(version.clone()),
        _ => None,
    };
    let pourcentage_telechargement = match &etat {
        EtatBanniereMaj::EnTelechargement { progression } => {
            Some((progression.clamp(0.0, 1.0) * 100.0).round() as i32)
        }
        _ => None,
    };
    let prete_a_installer = matches!(etat, EtatBanniereMaj::PreteAInstaller { .. });
    let erreur = match &etat {
        EtatBanniereMaj::Echouee(message) => Some(message.clone()),
        _ => None,
    };

    rsx! {
        if let Some(version) = disponible {
            div { class: "banniere-maj",
                span { class: "banniere-maj-texte", "Mise à jour {version} disponible." }
                div { class: "banniere-maj-actions",
                    button {
                        class: "bouton-principal",
                        onclick: move |_| sur_telecharger.call(()),
                        "Télécharger"
                    }
                    button {
                        class: "bouton-secondaire",
                        onclick: move |_| sur_ignorer.call(()),
                        "Ignorer"
                    }
                }
            }
        }
        if let Some(pourcentage) = pourcentage_telechargement {
            div { class: "banniere-maj",
                span { class: "banniere-maj-texte", "Téléchargement de la mise à jour… {pourcentage}%" }
                div { class: "barre-progression banniere-maj-barre",
                    div {
                        class: "barre-progression-remplissage",
                        style: "width: {pourcentage}%;",
                    }
                }
            }
        }
        if prete_a_installer {
            div { class: "banniere-maj",
                span { class: "banniere-maj-texte", "Mise à jour téléchargée, prête à installer." }
                div { class: "banniere-maj-actions",
                    button {
                        class: "bouton-principal",
                        onclick: move |_| sur_installer.call(()),
                        "Installer"
                    }
                    button {
                        class: "bouton-secondaire",
                        onclick: move |_| sur_ignorer.call(()),
                        "Ignorer"
                    }
                }
            }
        }
        if let Some(message) = erreur {
            div { class: "banniere-maj banniere-maj-erreur",
                span { class: "banniere-maj-texte", "Échec de la mise à jour : {message}" }
                button {
                    class: "bouton-secondaire",
                    onclick: move |_| sur_ignorer.call(()),
                    "Ignorer"
                }
            }
        }
    }
}
