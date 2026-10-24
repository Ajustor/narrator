use crate::de::De;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
