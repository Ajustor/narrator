use crate::caracteristiques::{modificateur, Caracteristiques};
use crate::de::De;
use crate::objet::{EffetObjet, Objet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Personnage {
    pub nom: String,
    pub classe: String,
    pub caracteristiques: Caracteristiques,
    pub pv_actuels: i32,
    pub pv_max: i32,
    pub inventaire: Vec<Objet>,
}

impl Personnage {
    pub fn nouveau(
        nom: String,
        classe: String,
        caracteristiques: Caracteristiques,
        inventaire: Vec<Objet>,
    ) -> Self {
        let pv_max = (10 + modificateur(caracteristiques.constitution)).max(1);
        Self {
            nom,
            classe,
            caracteristiques,
            pv_actuels: pv_max,
            pv_max,
            inventaire,
        }
    }

    pub fn defense(&self) -> i32 {
        10 + modificateur(self.caracteristiques.dexterite)
    }

    pub fn subir_degats(&mut self, degats: i32) {
        self.pv_actuels = (self.pv_actuels - degats).max(0);
    }

    pub fn soigner(&mut self, soin: i32) {
        self.pv_actuels = (self.pv_actuels + soin).min(self.pv_max);
    }

    pub fn est_mort(&self) -> bool {
        self.pv_actuels <= 0
    }

    pub fn arme_equipee(&self) -> De {
        self.inventaire
            .iter()
            .find_map(|o| match &o.effet {
                Some(EffetObjet::Arme { degats }) => Some(*degats),
                _ => None,
            })
            .unwrap_or(De { faces: 4 })
    }

    pub fn retirer_objet(&mut self, nom: &str) -> Option<Objet> {
        let position = self.inventaire.iter().position(|o| o.nom == nom)?;
        Some(self.inventaire.remove(position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objet::Objet;

    fn caracteristiques_test() -> Caracteristiques {
        Caracteristiques {
            force: 12,
            dexterite: 14,
            constitution: 13,
            intelligence: 10,
            sagesse: 10,
            charisme: 8,
        }
    }

    #[test]
    fn nouveau_calcule_pv_max_depuis_la_constitution() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.pv_max, 11); // 10 + modificateur(13) = 10 + 1
        assert_eq!(perso.pv_actuels, 11);
    }

    #[test]
    fn defense_depend_de_la_dexterite() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.defense(), 12); // 10 + modificateur(14) = 10 + 2
    }

    #[test]
    fn subir_degats_est_borne_a_zero() {
        let mut perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        perso.subir_degats(100);
        assert_eq!(perso.pv_actuels, 0);
        assert!(perso.est_mort());
    }

    #[test]
    fn soigner_est_borne_au_pv_max() {
        let mut perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        perso.subir_degats(5);
        perso.soigner(100);
        assert_eq!(perso.pv_actuels, perso.pv_max);
    }

    #[test]
    fn arme_equipee_retourne_mains_nues_sans_arme() {
        let perso = Personnage::nouveau("Elowen".into(), "Guerrier".into(), caracteristiques_test(), vec![]);
        assert_eq!(perso.arme_equipee(), De { faces: 4 });
    }

    #[test]
    fn arme_equipee_retourne_la_premiere_arme_trouvee() {
        let perso = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            caracteristiques_test(),
            vec![Objet::epee(), Objet::potion_de_soin()],
        );
        assert_eq!(perso.arme_equipee(), De { faces: 8 });
    }

    #[test]
    fn retirer_objet_enleve_et_retourne_l_objet() {
        let mut perso = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            caracteristiques_test(),
            vec![Objet::potion_de_soin()],
        );
        let retire = perso.retirer_objet("Potion de soin");
        assert!(retire.is_some());
        assert!(perso.inventaire.is_empty());
    }
}
