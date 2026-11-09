use crate::combat::Combat;
use crate::fiche_pnj::FichePnj;
use crate::personnage::Personnage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
    #[serde(default)]
    pub pnj_connus: Vec<FichePnj>,
    /// `false` tant que la toute première rencontre (actuellement le
    /// Gobelin de départ) n'a pas encore été déclenchée — voir
    /// `application::usecases::jouer_tour::amorcer_rencontre_initiale`.
    /// `#[serde(default)]` : une sauvegarde créée avant l'ajout de ce champ
    /// se relit avec `false`, ce qui redéclenche la rencontre à la
    /// prochaine action — acceptable pour une sauvegarde déjà ancienne,
    /// bien préférable à un champ manquant qui ferait échouer la lecture.
    #[serde(default)]
    pub rencontre_initiale_declenchee: bool,
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
            pnj_connus: vec![FichePnj {
                nom: "Aldric".into(),
                description: "forgeron bourru et méfiant".into(),
                historique: vec!["Première rencontre à la forge.".into()],
            }],
            rencontre_initiale_declenchee: true,
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
        assert_eq!(relu.pnj_connus, etat.pnj_connus);
    }
}
