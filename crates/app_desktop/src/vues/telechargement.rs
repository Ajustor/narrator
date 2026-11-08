use dioxus::prelude::*;

use crate::modeles::PaletteModele;

#[component]
pub fn Telechargement(palette: PaletteModele, progression: f32) -> Element {
    let pourcentage = (progression.clamp(0.0, 1.0) * 100.0).round() as i32;
    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Téléchargement du modèle" }
                p { "Téléchargement du modèle {palette.nom_affiche()} ({pourcentage}%)." }
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
