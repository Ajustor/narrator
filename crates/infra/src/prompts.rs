use application::ports::narrateur::{ContexteResultat, ContexteTour};
use domain::{ActionJoueur, EffetObjet, EtatPartie, FichePnj};

pub fn construire_prompt_classification(contexte: &ContexteTour) -> String {
    format!(
        "Tu es le maître du jeu d'un jeu de rôle solo en français.\n\
         Le joueur se trouve à : {}.\n\
         {}\
         {}\
         Le joueur dit : \"{}\"\n\
         Réponds uniquement par un objet JSON décrivant l'action correspondante, \
         sans aucun autre texte. Utilise exactement un de ces quatre formats, \
         avec ces clés exactes (remplace seulement la valeur après les deux-points \
         pour \"cible\"/\"objet\") :\n\
         {{\"action\": \"attaquer\", \"cible\": \"NomDeLEnnemi\"}}\n\
         {{\"action\": \"utiliser_objet\", \"objet\": \"NomDeLObjet\"}}\n\
         {{\"action\": \"fuir\"}}\n\
         {{\"action\": \"libre\"}}",
        contexte.etat.lieu_actuel,
        description_combat(&contexte.etat),
        description_personnage(&contexte.etat),
        contexte.saisie_joueur,
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
         Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         {}\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
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
         Réponds uniquement par un objet JSON, sans aucun autre texte : \
         {{\"pnj\": null}} si aucun, {{\"pnj\": \"Nom\", \"nouveau\": false}} si c'est un PNJ déjà \
         connu, ou {{\"pnj\": \"Nom\", \"nouveau\": true, \"description\": \"...\"}} si c'est un \
         nouveau PNJ (description en quelques mots : personnalité, rôle).",
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
        "Le joueur a {}/{} PV et possède {}.\n",
        etat.personnage.pv_actuels, etat.personnage.pv_max, inventaire,
    )
}

fn description_etat_apres_resolution(etat: &EtatPartie) -> String {
    let mut ligne = format!(
        "État actuel : le joueur a {}/{} PV restants.",
        etat.personnage.pv_actuels, etat.personnage.pv_max
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
    let ActionJoueur::ActionLibre(texte) = action else {
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
            .map(|p| format!("{} : {}", p.nom, p.description))
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                vec![],
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
        }
    }

    fn etat_en_combat() -> EtatPartie {
        let mut etat = etat_sans_combat();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
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
        let contexte = ContexteTour {
            etat: etat_sans_combat(),
            saisie_joueur: "J'observe".to_string(),
        };
        let prompt = construire_prompt_classification(&contexte);
        assert!(!prompt.contains("combat"));
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
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.is_empty());
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
            action: ActionJoueur::ActionLibre("J'observe".into()),
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
            action: ActionJoueur::ActionLibre("J'observe".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(prompt.contains("Elowen a quitté le village."));
    }

    #[test]
    fn le_prompt_de_narration_ne_mentionne_pas_de_memoire_si_resumes_est_vide() {
        let contexte = ContexteResultat {
            etat: etat_sans_combat(),
            action: ActionJoueur::ActionLibre("J'observe".into()),
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
            action: ActionJoueur::ActionLibre("Je parle à Aldric".into()),
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
            action: ActionJoueur::ActionLibre("Je regarde le ciel".into()),
            resume_mecanique: vec![],
        };
        let prompt = construire_prompt_narration(&contexte);
        assert!(!prompt.contains("Aldric"));
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
}
