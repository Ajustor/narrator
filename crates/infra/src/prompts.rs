use application::ports::narrateur::{ContexteResultat, ContexteTour};
use domain::EtatPartie;

pub fn construire_prompt_classification(contexte: &ContexteTour) -> String {
    format!(
        "Tu es le maître du jeu d'un jeu de rôle solo en français.\n\
         Le joueur se trouve à : {}.\n\
         {}\
         Le joueur dit : \"{}\"\n\
         Réponds uniquement par un objet JSON décrivant l'action correspondante, sans aucun autre texte.",
        contexte.etat.lieu_actuel,
        description_combat(&contexte.etat),
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
        "Le joueur a tenté : {:?}.\n\
         Voici ce qui s'est réellement passé : {}\n\
         Raconte cette scène de façon vivante et dramatique, en français, \
         en deux ou trois phrases, sans jamais contredire ces faits ni inventer d'autres chiffres.",
        contexte.action, resultat,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ActionJoueur, Caracteristiques, Combat, De, Ennemi, Personnage};

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
}
