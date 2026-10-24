use domain::{ActionJoueur, EffetObjet, EtatPartie};

#[derive(Debug, PartialEq)]
pub enum ErreurTour {
    AucunCombatEnCours,
    CibleInvalide,
    ObjetInvalide,
    EchecNarrateur,
}

pub fn valider_action(action: &ActionJoueur, etat: &EtatPartie) -> Result<(), ErreurTour> {
    match action {
        ActionJoueur::Attaquer { cible } => {
            let combat = etat.combat_en_cours.as_ref().ok_or(ErreurTour::AucunCombatEnCours)?;
            let trouve = combat.ennemis.iter().any(|e| &e.nom == cible && !e.est_mort());
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::CibleInvalide)
            }
        }
        ActionJoueur::UtiliserObjet { objet } => {
            let trouve = etat.personnage.inventaire.iter().any(|o| {
                &o.nom == objet && matches!(o.effet, Some(EffetObjet::Consommable { .. }))
            });
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::ObjetInvalide)
            }
        }
        ActionJoueur::Fuir => {
            if etat.combat_en_cours.is_some() {
                Ok(())
            } else {
                Err(ErreurTour::AucunCombatEnCours)
            }
        }
        ActionJoueur::ActionLibre(_) => Ok(()),
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
                vec![Objet::potion_de_soin()],
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
    fn attaquer_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn attaquer_une_cible_inexistante_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Dragon".into() }, &etat_en_combat());
        assert_eq!(resultat, Err(ErreurTour::CibleInvalide));
    }

    #[test]
    fn attaquer_une_cible_vivante_est_valide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn utiliser_un_objet_absent_est_invalide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion inexistante".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_arme_via_utiliser_objet_est_invalide() {
        let mut etat = etat_sans_combat();
        etat.personnage.inventaire.push(Objet::epee());
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Épée".into() }, &etat);
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_potion_presente_est_valide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn fuir_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn fuir_en_combat_est_valide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn action_libre_est_toujours_valide() {
        let resultat = valider_action(&ActionJoueur::ActionLibre("Je regarde autour de moi".into()), &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }
}
