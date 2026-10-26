use application::usecases::creer_personnage::{CLASSES_VALIDES, VALEURS_STANDARD};
use dioxus::prelude::*;

use crate::protocole::MessageVersWorker;

const NOMS_CARACTERISTIQUES: [&str; 6] =
    ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];

#[allow(dead_code)]
#[component]
pub fn Creation(sur_creation: EventHandler<MessageVersWorker>) -> Element {
    let mut nom = use_signal(String::new);
    let mut classe = use_signal(|| CLASSES_VALIDES[0].to_string());
    let mut valeurs = use_signal(|| [0u32; 6]);

    let valeurs_disponibles = move |index_a_exclure: usize| -> Vec<u32> {
        let deja_choisies: Vec<u32> = valeurs()
            .iter()
            .enumerate()
            .filter(|(i, v)| *i != index_a_exclure && **v != 0)
            .map(|(_, v)| *v)
            .collect();
        VALEURS_STANDARD
            .iter()
            .copied()
            .filter(|v| !deja_choisies.contains(v) || valeurs()[index_a_exclure] == *v)
            .collect()
    };

    let toutes_valeurs_choisies = move || valeurs().iter().all(|v| *v != 0);

    rsx! {
        div {
            h1 { "Création de personnage" }
            div {
                label { "Nom : " }
                input {
                    value: "{nom}",
                    oninput: move |evt| nom.set(evt.value()),
                }
            }
            div {
                label { "Classe : " }
                select {
                    value: "{classe}",
                    onchange: move |evt| classe.set(evt.value()),
                    for c in CLASSES_VALIDES.iter() {
                        option { key: "{c}", value: "{c}", "{c}" }
                    }
                }
            }
            for (index, label) in NOMS_CARACTERISTIQUES.iter().enumerate() {
                div {
                    label { "{label} : " }
                    select {
                        value: "{valeurs()[index]}",
                        onchange: move |evt| {
                            let mut v = valeurs();
                            v[index] = evt.value().parse().unwrap_or(0);
                            valeurs.set(v);
                        },
                        option { value: "0", "—" }
                        for valeur in valeurs_disponibles(index) {
                            option { key: "{valeur}", value: "{valeur}", "{valeur}" }
                        }
                    }
                }
            }
            button {
                disabled: !toutes_valeurs_choisies(),
                onclick: move |_| {
                    sur_creation.call(MessageVersWorker::CreerPersonnage {
                        nom: nom(),
                        classe: classe(),
                        valeurs: valeurs(),
                    });
                },
                "Commencer l'aventure"
            }
        }
    }
}
