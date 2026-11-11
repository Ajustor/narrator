use dioxus::prelude::*;
use domain::{EtatPartie, LigneTranscript};

use crate::protocole::{AffichageLigneTranscript, MessageVersWorker};

/// Un morceau de texte à afficher, et si c'est une action entre astérisques
/// (`*ainsi*` — voir la convention expliquée au modèle dans
/// `infra::prompts`) plutôt que de la parole ou de la narration ordinaire.
/// Ces actions sont décrites aussi bien par le joueur (dans sa saisie) que
/// par le narrateur (gestes d'un PNJ) : la distinction visuelle s'applique
/// donc aux deux côtés du journal.
struct SegmentTexte {
    texte: String,
    est_action: bool,
}

/// Découpe `texte` en alternant texte ordinaire et actions entre
/// astérisques, astérisques comprises (pour que l'action reste visuellement
/// délimitée même une fois grisée). Si le nombre d'astérisques est impair
/// (astérisque de fermeture oubliée par le modèle ou le joueur), le reste
/// du texte après la dernière astérisque est traité comme une action —
/// imperfection acceptée plutôt qu'une détection plus complexe pour un cas
/// rare et purement cosmétique.
fn decouper_actions(texte: &str) -> Vec<SegmentTexte> {
    texte
        .split('*')
        .enumerate()
        .filter(|(_, partie)| !partie.is_empty())
        .map(|(i, partie)| {
            let est_action = i % 2 == 1;
            SegmentTexte { texte: if est_action { format!("*{partie}*") } else { partie.to_string() }, est_action }
        })
        .collect()
}

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
                        p { class: "{ligne.classe()}",
                            for segment in decouper_actions(ligne.texte()) {
                                span {
                                    class: if segment.est_action { "action-texte" } else { "" },
                                    "{segment.texte}"
                                }
                            }
                        }
                    }
                    if !narration_en_cours.is_empty() {
                        p { class: "narration-en-cours",
                            for segment in decouper_actions(&narration_en_cours) {
                                span {
                                    class: if segment.est_action { "action-texte" } else { "" },
                                    "{segment.texte}"
                                }
                            }
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn textes(segments: &[SegmentTexte]) -> Vec<&str> {
        segments.iter().map(|s| s.texte.as_str()).collect()
    }

    fn actions(segments: &[SegmentTexte]) -> Vec<bool> {
        segments.iter().map(|s| s.est_action).collect()
    }

    #[test]
    fn decoupe_une_action_au_milieu_d_un_texte() {
        let segments = decouper_actions("Il dégaine son épée et *recule prudemment* vers la porte.");
        assert_eq!(textes(&segments), vec!["Il dégaine son épée et ", "*recule prudemment*", " vers la porte."]);
        assert_eq!(actions(&segments), vec![false, true, false]);
    }

    #[test]
    fn decoupe_un_texte_sans_action_en_un_seul_segment() {
        let segments = decouper_actions("Il observe les environs.");
        assert_eq!(textes(&segments), vec!["Il observe les environs."]);
        assert_eq!(actions(&segments), vec![false]);
    }

    #[test]
    fn decoupe_une_action_au_tout_debut_ou_a_la_toute_fin() {
        let segments = decouper_actions("*hausse les épaules* d'un air las");
        assert_eq!(textes(&segments), vec!["*hausse les épaules*", " d'un air las"]);
        assert_eq!(actions(&segments), vec![true, false]);
    }

    #[test]
    fn decoupe_plusieurs_actions_separees() {
        let segments = decouper_actions("*se penche* Qui va là ? *recule*");
        assert_eq!(textes(&segments), vec!["*se penche*", " Qui va là ? ", "*recule*"]);
        assert_eq!(actions(&segments), vec![true, false, true]);
    }

    #[test]
    fn traite_une_astersique_non_fermee_comme_une_action_jusqu_a_la_fin() {
        // Imperfection connue et acceptée : sans astérisque de fermeture, on
        // ne peut pas distinguer ce cas d'une action qui continue — mieux
        // vaut griser trop que planter ou perdre du texte.
        let segments = decouper_actions("Il recule *lentement vers le mur");
        assert_eq!(textes(&segments), vec!["Il recule ", "*lentement vers le mur*"]);
        assert_eq!(actions(&segments), vec![false, true]);
    }
}
