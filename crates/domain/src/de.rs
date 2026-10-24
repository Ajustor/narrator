use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct De {
    pub faces: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JetDe {
    pub resultat: u32,
    pub modificateur: i32,
    pub total: i32,
}

impl JetDe {
    pub fn nouveau(resultat: u32, modificateur: i32) -> Self {
        Self {
            resultat,
            modificateur,
            total: resultat as i32 + modificateur,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jet_de_calcule_le_total_avec_modificateur() {
        let jet = JetDe::nouveau(14, 2);
        assert_eq!(jet.resultat, 14);
        assert_eq!(jet.modificateur, 2);
        assert_eq!(jet.total, 16);
    }

    #[test]
    fn jet_de_total_peut_etre_negatif() {
        let jet = JetDe::nouveau(1, -3);
        assert_eq!(jet.total, -2);
    }
}
