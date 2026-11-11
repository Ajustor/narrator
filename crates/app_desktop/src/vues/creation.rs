use application::usecases::creer_personnage::CLASSES_VALIDES;
use dioxus::prelude::*;
use domain::Genre;

use crate::modeles::PaletteModele;
use crate::protocole::MessageVersWorker;

const NOMS_CARACTERISTIQUES: [&str; 6] =
    ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];

#[component]
pub fn Creation(
    valeurs_proposees: [u32; 6],
    /// `true` tant que la création est en cours côté worker (chargement du
    /// modèle puis génération de l'introduction, voir `main.rs`/`worker.rs`)
    /// — affiche un indicateur visuel et bloque un second envoi.
    chargement: bool,
    sur_creation: EventHandler<MessageVersWorker>,
) -> Element {
    let mut nom = use_signal(String::new);
    let mut classe = use_signal(|| CLASSES_VALIDES[0].to_string());
    // Pour chaque caractéristique (Force, Dextérité, ...), l'INDEX dans
    // `valeurs_proposees` qui lui a été attribué (pas la valeur elle-même).
    let mut origines = use_signal(|| [None::<usize>; 6]);
    let mut palette = use_signal(|| PaletteModele::Equilibre);
    let mut genre = use_signal(Genre::default);
    let mut premisse_libre = use_signal(String::new);

    // Les origines (indices dans `valeurs_proposees`) encore disponibles
    // pour la caractéristique `index_a_exclure` : tous les indices sauf
    // ceux déjà attribués à une AUTRE caractéristique ; l'origine déjà
    // choisie par `index_a_exclure` lui-même reste dans sa propre liste.
    //
    // On clé les <option> par cet index d'origine, stable, plutôt que par
    // la valeur tirée ou par une position dans une liste filtrée (ancienne
    // version) : retirer une valeur du pool partagé décalait les positions
    // des options restantes dans TOUS les <select>, ce qui forçait Dioxus
    // à recréer leurs nœuds <option> — et un <select> HTML réinitialise sa
    // sélection à la première option quand l'option sélectionnée est
    // recréée, sans que Dioxus ne réécrive `value` puisque cette prop-là
    // n'avait, elle, pas changé. Résultat observé : choisir une
    // caractéristique faisait disparaître celles déjà choisies. Avec une
    // clé stable par origine, retirer une origine ailleurs ne touche plus
    // aux options des autres <select>.
    let origines_disponibles = move |index_a_exclure: usize| -> Vec<usize> {
        let courantes = origines();
        (0..valeurs_proposees.len())
            .filter(|&o| {
                !courantes
                    .iter()
                    .enumerate()
                    .any(|(i, choix)| i != index_a_exclure && *choix == Some(o))
            })
            .collect()
    };

    let toutes_valeurs_choisies = move || origines().iter().all(|o| o.is_some());

    rsx! {
        div { class: "ecran-centre",
            div { class: "carte",
                h1 { "Création de personnage" }
                p { class: "texte-discret",
                    "Caractéristiques tirées pour cette aventure : {valeurs_proposees:?}. Répartis-les comme tu veux."
                }
                div { class: "champ",
                    label { "Nom : " }
                    input {
                        value: "{nom}",
                        oninput: move |evt| nom.set(evt.value()),
                    }
                }
                div { class: "champ",
                    label { "Classe : " }
                    select {
                        value: "{classe}",
                        onchange: move |evt| classe.set(evt.value()),
                        for c in CLASSES_VALIDES.iter() {
                            option { key: "{c}", value: "{c}", "{c}" }
                        }
                    }
                }
                div { class: "champ",
                    label { "Modèle : " }
                    select {
                        value: "{palette().nom_affiche()}",
                        onchange: move |evt| {
                            if let Some(p) = PaletteModele::TOUTES.iter().find(|p| p.nom_affiche() == evt.value()) {
                                palette.set(*p);
                            }
                        },
                        for p in PaletteModele::TOUTES.iter() {
                            option {
                                key: "{p.nom_affiche()}",
                                value: "{p.nom_affiche()}",
                                "{p.nom_affiche()} (~{crate::telechargement::formater_taille_go(p.taille_approx_octets())})"
                            }
                        }
                    }
                    if let Some(avertissement) = palette().avertissement() {
                        p { class: "avertissement", "{avertissement}" }
                    }
                }
                div { class: "champ",
                    label { "Genre : " }
                    select {
                        value: "{genre().nom_affiche()}",
                        onchange: move |evt| {
                            if let Some(g) = Genre::TOUTES.iter().find(|g| g.nom_affiche() == evt.value()) {
                                genre.set(*g);
                            }
                        },
                        for g in Genre::TOUTES.iter() {
                            option { key: "{g.nom_affiche()}", value: "{g.nom_affiche()}", "{g.nom_affiche()}" }
                        }
                    }
                }
                div { class: "champ",
                    label { "Prémisse (optionnel) : " }
                    textarea {
                        maxlength: "300",
                        value: "{premisse_libre}",
                        oninput: move |evt| premisse_libre.set(evt.value()),
                    }
                }
                for (index, label) in NOMS_CARACTERISTIQUES.iter().enumerate() {
                    div { class: "champ",
                        label { "{label} : " }
                        select {
                            value: match origines()[index] {
                                Some(o) => o.to_string(),
                                None => String::new(),
                            },
                            onchange: move |evt| {
                                let mut o = origines();
                                o[index] = evt.value().parse().ok();
                                origines.set(o);
                            },
                            option { value: "", "—" }
                            for origine in origines_disponibles(index) {
                                option { key: "{origine}", value: "{origine}", "{valeurs_proposees[origine]}" }
                            }
                        }
                    }
                }
                button {
                    class: "bouton-principal",
                    disabled: chargement || nom().trim().is_empty() || !toutes_valeurs_choisies(),
                    onclick: move |_| {
                        let valeurs = origines().map(|o| o.map(|i| valeurs_proposees[i]).unwrap_or(0));
                        sur_creation.call(MessageVersWorker::CreerPersonnage {
                            nom: nom().trim().to_string(),
                            classe: classe(),
                            valeurs,
                            palette_modele: palette(),
                            genre: genre(),
                            premisse_libre: premisse_libre(),
                        });
                    },
                    "Commencer l'aventure"
                }
                if chargement {
                    div { class: "chargement-ligne",
                        span { class: "spinner" }
                        span { "Création de l'histoire en cours…" }
                    }
                }
            }
        }
    }
}
