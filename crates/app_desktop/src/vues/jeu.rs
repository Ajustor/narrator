use dioxus::prelude::*;
use domain::{EtatPartie, LigneTranscript};

use crate::protocole::{AffichageLigneTranscript, MessageVersWorker};

#[component]
pub fn Jeu(
    etat: EtatPartie,
    journal: Vec<LigneTranscript>,
    narration_en_cours: String,
    tour_en_cours: bool,
    /// `Some((faces, etiquette))` tant que le worker attend que le joueur
    /// lance son propre dé (attaque ou dégâts) — voir
    /// `worker::GenerateurDeJoueur`. Les jets des ennemis restent
    /// automatiques et ne passent jamais par ici.
    de_en_attente: Option<(u32, String)>,
    /// Résultat réel du dé une fois lancé, affiché sur le dé avant même
    /// que la narration complète du tour arrive.
    dernier_resultat_de: Option<u32>,
    sur_action: EventHandler<MessageVersWorker>,
    sur_retour_selection: EventHandler<()>,
    sur_lancer_de: EventHandler<()>,
) -> Element {
    let mut saisie = use_signal(String::new);
    let mut pnj_deplie: Signal<Option<String>> = use_signal(|| None);

    let mut envoyer = move || {
        if !saisie().trim().is_empty() && !tour_en_cours {
            sur_action.call(MessageVersWorker::JouerTour(saisie()));
            saisie.set(String::new());
        }
    };

    rsx! {
        if let Some((faces, etiquette)) = de_en_attente.clone() {
            div { class: "voile-de",
                div { class: "dialogue-de",
                    p { class: "etiquette-de", "{etiquette}" }
                    if let Some(valeur) = dernier_resultat_de {
                        div { class: "de-resultat", "{valeur}" }
                        p { class: "texte-discret", "d{faces}" }
                    } else {
                        button {
                            class: "de-bouton",
                            onclick: move |_| sur_lancer_de.call(()),
                            "d{faces}"
                        }
                        p { class: "texte-discret", "Touche le dé pour lancer" }
                    }
                }
            }
        }
        div { class: "ecran-jeu",
            div { class: "zone-principale",
                header { class: "en-tete",
                    button {
                        class: "bouton-retour",
                        disabled: tour_en_cours,
                        onclick: move |_| sur_retour_selection.call(()),
                        "← Histoires"
                    }
                    span { class: "lieu", "{etat.lieu_actuel}" }
                    span { "PV : {etat.personnage.pv_actuels}/{etat.personnage.pv_max}" }
                }

                if let Some(combat) = etat.combat_en_cours.as_ref() {
                    if !combat.termine {
                        div { class: "panneau-combat",
                            h3 { "Combat" }
                            for ennemi in combat.ennemis.iter().filter(|e| !e.est_mort()) {
                                div {
                                    key: "{ennemi.nom}",
                                    class: "ennemi",
                                    "{ennemi.nom} : {ennemi.pv_actuels}/{ennemi.pv_max} PV"
                                }
                            }
                        }
                    }
                }

                div { id: "journal", class: "journal",
                    for ligne in journal.iter() {
                        p { class: "{ligne.classe()}", "{ligne.texte()}" }
                    }
                    if !narration_en_cours.is_empty() {
                        p { class: "narration-en-cours", "{narration_en_cours}" }
                    } else if tour_en_cours {
                        p { class: "narration-en-cours indicateur-attente", "Le narrateur réfléchit…" }
                    }
                }

                div { class: "zone-saisie",
                    input {
                        value: "{saisie}",
                        disabled: tour_en_cours,
                        oninput: move |evt| saisie.set(evt.value()),
                        onkeydown: move |evt| {
                            if evt.key() == Key::Enter {
                                envoyer();
                            }
                        },
                    }
                    button {
                        class: "bouton-principal",
                        disabled: tour_en_cours,
                        onclick: move |_| envoyer(),
                        "Envoyer"
                    }
                }
            }

            if !etat.pnj_connus.is_empty() {
                aside { class: "panneau-pnj",
                    h3 { "PNJ rencontrés" }
                    for pnj in etat.pnj_connus.iter() {
                        div {
                            key: "{pnj.nom}",
                            class: "pnj",
                            onclick: {
                                let nom = pnj.nom.clone();
                                move |_| {
                                    let actuel = pnj_deplie();
                                    pnj_deplie.set(if actuel.as_deref() == Some(nom.as_str()) { None } else { Some(nom.clone()) });
                                }
                            },
                            strong { "{pnj.nom}" }
                            span { " — {pnj.description}" }
                            if pnj_deplie().as_deref() == Some(pnj.nom.as_str()) {
                                ul {
                                    for entree in pnj.historique.iter() {
                                        li { "{entree}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
