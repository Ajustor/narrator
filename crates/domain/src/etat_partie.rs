use crate::combat::Combat;
use crate::personnage::Personnage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caracteristiques::Caracteristiques;

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
            resumes: vec!["Le voyage commence.".into()],
        }
    }

    #[test]
    fn etat_partie_fait_un_aller_retour_json() {
        let etat = etat_test();
        let json = serde_json::to_string(&etat).expect("sérialisation échouée");
        let relu: EtatPartie = serde_json::from_str(&json).expect("désérialisation échouée");
        assert_eq!(relu.personnage.nom, etat.personnage.nom);
        assert_eq!(relu.lieu_actuel, etat.lieu_actuel);
        assert_eq!(relu.resumes, etat.resumes);
    }
}
