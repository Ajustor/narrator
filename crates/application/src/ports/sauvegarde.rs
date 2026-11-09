use domain::EtatPartie;

#[derive(Debug)]
pub enum ErreurSauvegarde {
    FichierAbsent,
    Lecture(String),
    Ecriture(String),
}

pub trait Sauvegarde {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde>;
    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde>;
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
            self.contenu
                .borrow()
                .clone()
                .ok_or(ErreurSauvegarde::FichierAbsent)
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
                Caracteristiques {
                    force: 12,
                    dexterite: 14,
                    constitution: 13,
                    intelligence: 10,
                    sagesse: 10,
                    charisme: 8,
                },
                vec![],
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
            rencontre_initiale_declenchee: true,
        }
    }

    #[test]
    fn charger_sans_sauvegarde_prealable_echoue() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        assert!(matches!(sauvegarde.charger(), Err(ErreurSauvegarde::FichierAbsent)));
    }

    #[test]
    fn sauvegarder_puis_charger_renvoie_le_meme_etat() {
        let sauvegarde = SauvegardeMemoire { contenu: RefCell::new(None) };
        let etat = etat_test();
        sauvegarde.sauvegarder(&etat).unwrap();
        let relu = sauvegarde.charger().unwrap();
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
    }
}
