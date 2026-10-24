use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caracteristiques {
    pub force: u32,
    pub dexterite: u32,
    pub constitution: u32,
    pub intelligence: u32,
    pub sagesse: u32,
    pub charisme: u32,
}

pub fn modificateur(valeur: u32) -> i32 {
    (valeur as i32 - 10).div_euclid(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modificateur_de_quinze_est_plus_deux() {
        assert_eq!(modificateur(15), 2);
    }

    #[test]
    fn modificateur_de_dix_est_zero() {
        assert_eq!(modificateur(10), 0);
    }

    #[test]
    fn modificateur_de_huit_est_moins_un() {
        assert_eq!(modificateur(8), -1);
    }
}
