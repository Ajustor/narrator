use crate::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use domain::EtatPartie;

pub fn sauvegarder_partie(sauvegarde: &dyn Sauvegarde, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
    sauvegarde.sauvegarder(etat)
}

pub fn charger_partie(sauvegarde: &dyn Sauvegarde) -> Result<EtatPartie, ErreurSauvegarde> {
    sauvegarde.charger()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Personnage};
    use std::cell::RefCell;

    struct SauvegardeMemoire {
        contenu: RefCell<Option<EtatPartie>>,
    }

    impl Sauvegarde for SauvegardeMemoire {
        fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
            self.contenu.borrow().clone().ok_or(ErreurSauvegarde::FichierAbsent)
        }

        fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
            *self.contenu.borrow_mut() = Some(etat.clone());
            Ok(())
        }
    }

    fn etat_test() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                vec![],
            ),
            premisse: Default::default(),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
        }
    }

    #[test]
    fn sauvegarder_puis_charger_fait_un_aller_retour() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        let etat = etat_test();
        sauvegarder_partie(&sauvegarde, &etat).unwrap();
        let relu = charger_partie(&sauvegarde).unwrap();
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
    }

    #[test]
    fn charger_sans_sauvegarde_propage_l_erreur() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        assert!(matches!(charger_partie(&sauvegarde), Err(ErreurSauvegarde::FichierAbsent)));
    }
}
