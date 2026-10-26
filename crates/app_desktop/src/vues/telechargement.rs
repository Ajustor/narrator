use dioxus::prelude::*;

#[component]
pub fn Telechargement(progression: f32) -> Element {
    let pourcentage = (progression.clamp(0.0, 1.0) * 100.0).round() as i32;
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Téléchargement du modèle" }
                p { "Premier lancement : téléchargement du modèle de narration ({pourcentage} %)." }
                div { class: "barre-progression",
                    div {
                        class: "barre-progression-remplissage",
                        style: "width: {pourcentage}%;",
                    }
                }
            }
        }
    }
}
