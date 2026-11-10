use crate::combat::Combat;
use crate::fiche_pnj::FichePnj;
use crate::personnage::Personnage;
use crate::premisse::PremisseHistoire;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtatPartie {
    pub personnage: Personnage,
    pub lieu_actuel: String,
    pub combat_en_cours: Option<Combat>,
    pub resumes: Vec<String>,
    #[serde(default)]
    pub pnj_connus: Vec<FichePnj>,
    #[serde(default)]
    pub premisse: PremisseHistoire,
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
            premisse: PremisseHistoire::default(),
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
        assert_eq!(relu.premisse, etat.premisse);
    }

    #[test]
    fn une_sauvegarde_sans_champ_premisse_se_relit_avec_la_valeur_par_defaut() {
        let etat = etat_test();
        let mut valeur = serde_json::to_value(&etat).unwrap();
        valeur.as_object_mut().unwrap().remove("premisse"); // simule une sauvegarde plus ancienne
        let relu: EtatPartie = serde_json::from_value(valeur).unwrap();
        assert_eq!(relu.premisse, PremisseHistoire::default());
    }
}
