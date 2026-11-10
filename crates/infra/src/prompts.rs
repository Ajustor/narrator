use application::ports::narrateur::{ContexteResultat, ContexteTour};
use domain::{ActionJoueur, EffetObjet, EtatPartie, FichePnj, Personnage, PremisseHistoire};

const LONGUEUR_MAX_PREMISSE: usize = 300;

pub fn construire_prompt_introduction(personnage: &Personnage, lieu: &str, premisse: &PremisseHistoire) -> String {
    let description = if premisse.description.is_empty() {
        String::new()
    } else {
        format!(" {}", tronquer(&premisse.description, LONGUEUR_MAX_PREMISSE))
    };
    format!(
        "Tu es le maître du jeu d'un jeu de rôle solo en français.\n\
         L'histoire se déroule dans {}.{}\n\
         Un·e {} nommé·e {} arrive à {} pour commencer son aventure.\n\
         Décris cette arrivée et plante le décor du monde en français, en trois ou \
         quatre phrases, sans combat, sans dialogue et sans inventer de personnage \
         non-joueur : seulement l'ambiance, le lieu et ce qui s'y trouve.",
        premisse.genre.indication_prompt(), description, personnage.classe, personnage.nom, lieu,
    )
}

pub fn construire_prompt_classification(contexte: &ContexteTour) -> String {
    let combat_actif = contexte.etat.combat_en_cours.as_ref().map(|c| !c.termine).unwrap_or(false);
    let (exemple_libre, instruction_agressif): (&str, &str) = if combat_actif {
        (
            "{\"action\": \"libre\", \"agressif\": true|false}",
            "Pour une action libre en combat, \"agressif\": true signifie une action \
             physiquement hostile (frapper, lancer quelque chose, charger), et \
             \"agressif\": false signifie une action non-violente (parler, négocier, \
             observer, se cacher...).\n",
        )
    } else {
        ("{\"action\": \"libre\"}", "")
    };
    let (exemple_provoquer_combat, instruction_provoquer_combat): (&str, &str) = if combat_actif {
        ("", "")
    } else {
        (
            "{\"action\": \"provoquer_combat\", \"ennemi\": \"NomDeLEnnemi\", \"difficulte\": \"faible\"|\"moyen\"|\"fort\"}\n",
            "Utilise \"provoquer_combat\" seulement si l'action du joueur est hostile \
             envers une cible nommée, ou si la situation appelle une rencontre hostile \
             soudaine. Choisis \"faible\" par défaut pour la difficulté, sauf si le \
             joueur a clairement attaqué ou si l'histoire a déjà escaladé.\n",
        )
    };
    format!(
        "Tu es le maître du jeu d'un jeu de rôle solo en français.\n\
         Le joueur se trouve à : {}.\n\
         {}\
         {}\
         Le joueur dit : \"{}\"\n\
         Dans sa saisie, le texte entre astérisques (*ainsi*) décrit un geste ou une action \
         physique du joueur, et le texte sans astérisques est ce qu'il dit à voix haute ; \
         tiens compte des deux sans les confondre (une action physique n'est pas une réplique).\n\
         Réponds uniquement par un objet JSON décrivant l'action correspondante, \
         sans aucun autre texte. Utilise exactement un de ces formats, \
         avec ces clés exactes (remplace seulement la valeur après les deux-points \
         pour \"cible\"/\"objet\"/\"ennemi\"/\"difficulte\") :\n\
         {{\"action\": \"attaquer\", \"cible\": \"NomDeLEnnemi\"}}\n\
         {{\"action\": \"utiliser_objet\", \"objet\": \"NomDeLObjet\"}}\n\
         {{\"action\": \"fuir\"}}\n\
         {{\"action\": \"esquiver\"}}\n\
         {}\
         {}\n\
         {}\
         {}",
        contexte.etat.lieu_actuel,
        description_combat(&contexte.etat),
        description_personnage(&contexte.etat),
        contexte.saisie_joueur,
        exemple_provoquer_combat,
        exemple_libre,
        instruction_agressif,
        instruction_provoquer_combat,
    )
}

pub fn construire_prompt_narration(contexte: &ContexteResultat) -> String {
    let resultat = if contexte.resume_mecanique.is_empty() {
        "Rien de mécanique ne s'est produit.".to_string()
    } else {
        contexte.resume_mecanique.join(" ")
    };
    format!(
        "{}\
         {}\
         {}\
         Le joueur a tenté : {:?}.\n\
         Dans sa saisie, le texte entre astérisques (*ainsi*) décrit un geste ou une action \
         physique du joueur, et le texte sans astérisques est ce qu'il dit à voix haute ; \
         ne transforme pas une action physique en réplique citée.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        description_premisse(&contexte.etat),
        description_memoire(&contexte.etat),
        description_pnj_pertinents(&contexte.etat, &contexte.action),
        contexte.action,
        resultat,
        description_etat_apres_resolution(&contexte.etat),
    )
}

pub fn construire_prompt_condensation(anciens: &[String]) -> String {
    format!(
        "Voici plusieurs résumés successifs d'une histoire de jeu de rôle en français :\n\
         {}\n\
         Condense-les en un seul paragraphe court (3-4 phrases maximum) en français, \
         qui garde les faits importants (lieux visités, événements marquants, objets \
         obtenus, ennemis vaincus) sans les détails mineurs. Réponds uniquement par ce \
         paragraphe, sans aucun autre texte.",
        anciens.join("\n"),
    )
}

pub fn construire_prompt_detection_pnj(narration: &str, noms_connus: &[String]) -> String {
    format!(
        "Voici ce qui vient d'être raconté : {}\n\
         PNJ déjà connus : {}\n\
         Un personnage non-joueur important est-il apparu ou a-t-il été mentionné dans ce texte ? \
         Réponds uniquement par un objet JSON, sans aucun autre texte. Utilise exactement un de ces \
         deux formats, avec ces clés exactes :\n\
         {{\"pnj\": null}}\n\
         {{\"pnj\": \"Aldric\", \"description\": \"forgeron bourru et méfiant\"}}\n\
         Utilise le premier format si aucun PNJ important n'est apparu ou mentionné. Utilise le \
         deuxième format sinon — remplace \"Aldric\" par son nom réel. Si ce PNJ figure déjà dans \
         la liste des PNJ déjà connus ci-dessus, tu peux laisser \"description\" vide (\"\") ; \
         sinon, donne une description de quelques mots (personnalité, rôle).",
        narration,
        if noms_connus.is_empty() { "aucun".to_string() } else { noms_connus.join(", ") },
    )
}

fn description_personnage(etat: &EtatPartie) -> String {
    let consommables: Vec<&str> = etat
        .personnage
        .inventaire
        .iter()
        .filter(|o| matches!(o.effet, Some(EffetObjet::Consommable { .. })))
        .map(|o| o.nom.as_str())
        .collect();

    let inventaire = if consommables.is_empty() {
        "aucun objet consommable".to_string()
    } else {
        format!("objets consommables : {}", consommables.join(", "))
    };

    format!(
        "Le joueur se nomme {} et a {}/{} PV ; il possède {}.\n",
        etat.personnage.nom, etat.personnage.pv_actuels, etat.personnage.pv_max, inventaire,
    )
}

fn description_etat_apres_resolution(etat: &EtatPartie) -> String {
    let mut ligne = format!(
        "État actuel : {} a {}/{} PV restants.",
        etat.personnage.nom, etat.personnage.pv_actuels, etat.personnage.pv_max
    );

    if let Some(combat) = &etat.combat_en_cours {
        if !combat.termine {
            let survivants: Vec<String> = combat
                .ennemis
                .iter()
                .filter(|e| !e.est_mort())
                .map(|e| format!("{} ({}/{} PV)", e.nom, e.pv_actuels, e.pv_max))
                .collect();
            if !survivants.is_empty() {
                ligne.push_str(&format!(" Ennemis restants : {}.", survivants.join(", ")));
            }
        }
    }

    ligne.push('\n');
    ligne
}

fn description_combat(etat: &EtatPartie) -> String {
    match &etat.combat_en_cours {
        Some(combat) if !combat.termine => {
            let noms: Vec<&str> = combat
                .ennemis
                .iter()
                .filter(|e| !e.est_mort())
                .map(|e| e.nom.as_str())
                .collect();
            if noms.is_empty() {
                String::new()
            } else {
                format!("Il est en combat contre : {}.\n", noms.join(", "))
            }
        }
        _ => String::new(),
    }
}

fn description_pnj_pertinents(etat: &EtatPartie, action: &ActionJoueur) -> String {
    let ActionJoueur::ActionLibre { texte, .. } = action else {
        return String::new();
    };
    let texte_minuscule = texte.to_lowercase();
    let pertinents: Vec<&FichePnj> = etat
        .pnj_connus
        .iter()
        .filter(|p| texte_minuscule.contains(&p.nom.to_lowercase()))
        .collect();
    if pertinents.is_empty() {
        String::new()
    } else {
        let descriptions: Vec<String> = pertinents
            .iter()
            .map(|p| {
                if p.description.is_empty() {
                    p.nom.clone()
                } else {
                    format!("{} : {}", p.nom, p.description)
                }
            })
            .collect();
        format!("PNJ pertinents déjà rencontrés : {}\n", descriptions.join(" ; "))
    }
}

fn description_memoire(etat: &EtatPartie) -> String {
    if etat.resumes.is_empty() {
        String::new()
    } else {
        format!("Ce qui s'est passé jusqu'ici : {}\n", etat.resumes.join(" "))
    }
}

/// Rappelle à chaque tour le genre et (si renseignée) la description libre
/// de la prémisse choisie à la création de la partie, pour que le ton de
/// l'histoire reste cohérent sur la durée, pas seulement à l'introduction.
fn description_premisse(etat: &EtatPartie) -> String {
    if etat.premisse.description.is_empty() {
        format!("Contexte de l'histoire : {}.\n", etat.premisse.genre.indication_prompt())
    } else {
        format!(
            "Contexte de l'histoire : {}. {}\n",
            etat.premisse.genre.indication_prompt(),
            tronquer(&etat.premisse.description, LONGUEUR_MAX_PREMISSE),
        )
    }
}

/// Tronque `s` à au plus `max` caractères (pas octets, pour rester valide
/// sur une frontière UTF-8 même avec des caractères accentués).
fn tronquer(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Genre, Objet, Personnage};

    fn personnage_test() -> Personnage {
        Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
            vec![],
        )
    }

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: personnage_test(),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
            premisse: Default::default(),
        }
    }

    fn etat_en_combat() -> EtatPartie {
        let mut etat = etat_sans_combat();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        etat
    }

    #[test]
    fn le_prompt_de_classification_contient_la_saisie_du_joueur() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "Je regarde autour de moi".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Je regarde autour de moi"));
    }

    #[test]
    fn le_prompt_de_classification_mentionne_les_ennemis_en_combat() {
        let contexte = ContexteTour {
            etat: etat_en_combat(),
            saisie_joueur: "J'attaque".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Gobelin"));
    }

    #[test]
    fn le_prompt_de_classification_ne_mentionne_pas_de_combat_hors_combat() {
        // Ne vérifie plus l'absence pure et simple du mot "combat" : le
        // format "provoquer_combat" (nouveau mécanisme de déclenchement
        // dynamique) est désormais ajouté précisément dans cette branche
        // hors-combat, et contient ce mot dans son nom. Ce qui reste
        // interdit ici, c'est la description d'un combat déjà actif
        // (`description_combat`), absurde hors combat.
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(!prompt.contains("Il est en combat contre"));
    }

    #[test]
    fn le_prompt_de_classification_demande_l_agressivite_d_une_action_libre_en_combat() {
        let contexte = ContexteTour {
            etat: etat_en_combat(),
            saisie_joueur: "Je lui jette du sable".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains(r#""agressif""#));
        assert!(prompt.contains("hostile"));
    }

    #[test]
    fn le_prompt_de_classification_ne_demande_pas_l_agressivite_hors_combat() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(!prompt.contains("agressif"));
    }

    #[test]
    fn le_prompt_de_narration_contient_le_resume_mecanique() {
        let contexte = ContexteResultat {
            etat: etat_en_combat(),
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            resume_mecanique: vec!["🎲 Tu attaques Gobelin : touché ! Dégâts : 5".to_string()],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("touché"));
        assert!(prompt.contains("Dégâts : 5"));
    }

    #[test]
    fn le_prompt_de_narration_gere_un_resume_vide_sans_paniquer() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.is_empty());
    }

    #[test]
    fn le_prompt_de_classification_mentionne_le_nom_du_personnage() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Elowen"));
    }

    #[test]
    fn le_prompt_de_narration_mentionne_le_nom_du_personnage() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Elowen"));
    }

    #[test]
    fn le_prompt_de_classification_explique_la_convention_asterisque() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "*je dégaine mon épée*".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("astérisque"));
    }

    #[test]
    fn le_prompt_de_narration_explique_la_convention_asterisque() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre { texte: "*je dégaine mon épée*".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("astérisque"));
    }

    #[test]
    fn le_prompt_de_classification_mentionne_les_pv_du_joueur() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        let pv = contexte.etat.personnage.pv_actuels;
        assert!(prompt.contains(&format!("{}/{}", pv, contexte.etat.personnage.pv_max)));
    }

    #[test]
    fn le_prompt_de_classification_contient_un_exemple_pour_chaque_format_d_action() {
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains(r#"{"action": "attaquer", "cible": "NomDeLEnnemi"}"#));
        assert!(prompt.contains(r#"{"action": "utiliser_objet", "objet": "NomDeLObjet"}"#));
        assert!(prompt.contains(r#"{"action": "fuir"}"#));
        assert!(prompt.contains(r#"{"action": "esquiver"}"#));
        assert!(prompt.contains(r#"{"action": "libre"}"#));
    }

    #[test]
    fn le_prompt_de_classification_mentionne_un_objet_consommable_en_inventaire() {
        let mut etat = etat_sans_combat();
        etat.personnage.inventaire.push(Objet::potion_de_soin());
        let contexte = ContexteTour {
            etat,
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(prompt.contains("Potion de soin"));
    }

    #[test]
    fn le_prompt_de_narration_mentionne_les_pv_restants_du_joueur() {
        let mut etat = etat_sans_combat();
        etat.personnage.subir_degats(3);
        let contexte = ContexteResultat {
            etat: etat.clone(),
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains(&format!("{}/{}", etat.personnage.pv_actuels, etat.personnage.pv_max)));
    }

    #[test]
    fn le_prompt_de_narration_mentionne_les_pv_d_un_ennemi_survivant() {
        let etat = etat_en_combat();
        let contexte = ContexteResultat {
            etat: etat.clone(),
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            resume_mecanique: vec!["🎲 Tu attaques Gobelin : touché ! Dégâts : 2".to_string()],
        };
        let prompt = construire_prompt_narration(&contexte);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert!(prompt.contains(&format!("{} ({}/{} PV)", gobelin.nom, gobelin.pv_actuels, gobelin.pv_max)));
    }

    #[test]
    fn le_prompt_de_condensation_contient_les_resumes_fournis() {
        let anciens = vec!["Elowen quitte le village.".to_string(), "Elowen combat un gobelin.".to_string()];
        let prompt = construire_prompt_condensation(&anciens);
        assert!(prompt.contains("Elowen quitte le village."));
        assert!(prompt.contains("Elowen combat un gobelin."));
    }

    #[test]
    fn le_prompt_de_condensation_demande_un_resultat_court_et_en_francais() {
        let prompt = construire_prompt_condensation(&["Un résumé.".to_string()]);
        assert!(prompt.contains("en français"));
        assert!(prompt.contains("court"));
    }

    #[test]
    fn le_prompt_de_narration_contient_les_resumes_accumules() {
        let mut etat = etat_sans_combat();
        etat.resumes = vec!["Elowen a quitté le village.".to_string()];
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Elowen a quitté le village."));
    }

    #[test]
    fn le_prompt_de_narration_ne_mentionne_pas_de_memoire_si_resumes_est_vide() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.contains("Ce qui s'est passé jusqu'ici"));
    }

    #[test]
    fn le_prompt_de_narration_contient_la_fiche_d_un_pnj_mentionne_par_le_joueur() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru et méfiant".to_string(),
            historique: vec![],
        });
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre { texte: "Je parle à Aldric".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Aldric"));
        assert!(prompt.contains("forgeron bourru et méfiant"));
    }

    #[test]
    fn le_prompt_de_narration_ne_mentionne_pas_un_pnj_non_pertinent() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru".to_string(),
            historique: vec![],
        });
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre { texte: "Je regarde le ciel".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.contains("Aldric"));
    }

    #[test]
    fn le_prompt_de_narration_n_affiche_pas_de_deux_points_vide_pour_un_pnj_sans_description() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: String::new(),
            historique: vec![],
        });
        let contexte = ContexteResultat {
            etat,
            action: ActionJoueur::ActionLibre { texte: "Je parle à Aldric".into(), agressif: false },
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Aldric"));
        assert!(!prompt.contains("Aldric : "));
    }

    #[test]
    fn le_prompt_de_detection_pnj_contient_la_narration_et_les_noms_connus() {
        let prompt = construire_prompt_detection_pnj("Aldric te salue.", &["Aldric".to_string()]);
        assert!(prompt.contains("Aldric te salue."));
        assert!(prompt.contains("Aldric"));
    }

    #[test]
    fn le_prompt_de_detection_pnj_gere_une_liste_de_noms_connus_vide() {
        let prompt = construire_prompt_detection_pnj("Rien ne se passe.", &[]);
        assert!(prompt.contains("aucun"));
    }

    #[test]
    fn le_prompt_de_detection_pnj_contient_un_exemple_pour_chaque_format() {
        let prompt = construire_prompt_detection_pnj("Peu importe.", &[]);
        assert!(prompt.contains(r#"{"pnj": null}"#));
        assert!(prompt.contains(r#"{"pnj": "Aldric", "description": "forgeron bourru et méfiant"}"#));
    }

    #[test]
    fn le_prompt_d_introduction_mentionne_le_genre_et_la_premisse() {
        let premisse = PremisseHistoire { genre: Genre::Horreur, description: "une vieille malédiction".into() };
        let prompt = construire_prompt_introduction(&personnage_test(), "Clairière", &premisse);
        assert!(prompt.contains("une vieille malédiction"));
    }

    #[test]
    fn le_prompt_de_narration_rappelle_la_premisse_a_chaque_tour() {
        let mut contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
        };
        contexte.etat.premisse = PremisseHistoire { genre: Genre::Horreur, description: "une vieille malédiction".into() };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("une vieille malédiction"));
    }

    #[test]
    fn la_premisse_est_tronquee_a_300_caracteres_dans_le_prompt() {
        let longue = "x".repeat(400);
        let premisse = PremisseHistoire { genre: Genre::FantasyMedievale, description: longue };
        let prompt = construire_prompt_introduction(&personnage_test(), "Clairière", &premisse);
        assert!(!prompt.contains(&"x".repeat(301)));
    }
}
