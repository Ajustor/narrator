use crate::de::De;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulte {
    Faible,
    Moyen,
    Fort,
}

pub fn gabarit_ennemi(nom: String, difficulte: Difficulte) -> Ennemi {
    match difficulte {
        Difficulte::Faible => Ennemi::nouveau(nom, 4, De { faces: 4 }, 10, 1, 1),
        Difficulte::Moyen => Ennemi::nouveau(nom, 7, De { faces: 6 }, 12, 2, 3),
        Difficulte::Fort => Ennemi::nouveau(nom, 12, De { faces: 8 }, 14, 3, 5),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ennemi {
    pub nom: String,
    pub pv_actuels: i32,
    pub pv_max: i32,
    pub degats: De,
    pub defense: i32,
    pub bonus_initiative: i32,
    pub bonus_attaque: i32,
}

impl Ennemi {
    pub fn nouveau(
        nom: String,
        pv_max: i32,
        degats: De,
        defense: i32,
        bonus_initiative: i32,
        bonus_attaque: i32,
    ) -> Self {
        Self {
            nom,
            pv_actuels: pv_max,
            pv_max,
            degats,
            defense,
            bonus_initiative,
            bonus_attaque,
        }
    }

    pub fn subir_degats(&mut self, degats: i32) {
        self.pv_actuels = (self.pv_actuels - degats).max(0);
    }

    pub fn est_mort(&self) -> bool {
        self.pv_actuels <= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::De;

    #[test]
    fn nouveau_initialise_les_pv_actuels_au_max() {
        let gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        assert_eq!(gobelin.pv_actuels, 7);
        assert_eq!(gobelin.pv_max, 7);
    }

    #[test]
    fn subir_degats_est_borne_a_zero() {
        let mut gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        gobelin.subir_degats(100);
        assert_eq!(gobelin.pv_actuels, 0);
        assert!(gobelin.est_mort());
    }

    #[test]
    fn gabarit_ennemi_faible_a_les_stats_du_palier_faible() {
        let e = gabarit_ennemi("Rat".into(), Difficulte::Faible);
        assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
            (4, De { faces: 4 }, 10, 1, 1));
    }

    #[test]
    fn gabarit_ennemi_moyen_reprend_les_stats_de_l_ancien_gobelin() {
        let e = gabarit_ennemi("Gobelin".into(), Difficulte::Moyen);
        assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
            (7, De { faces: 6 }, 12, 2, 3));
    }

    #[test]
    fn gabarit_ennemi_fort_a_les_stats_du_palier_fort() {
        let e = gabarit_ennemi("Ogre".into(), Difficulte::Fort);
        assert_eq!((e.pv_max, e.degats, e.defense, e.bonus_initiative, e.bonus_attaque),
            (12, De { faces: 8 }, 14, 3, 5));
    }

    #[test]
    fn gabarit_ennemi_garde_le_nom_donne() {
        assert_eq!(gabarit_ennemi("Loup".into(), Difficulte::Faible).nom, "Loup");
    }
}
