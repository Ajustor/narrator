use crate::de::De;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EffetObjet {
    Arme { degats: De },
    Consommable { soin: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objet {
    pub nom: String,
    pub description: String,
    pub effet: Option<EffetObjet>,
}

impl Objet {
    pub fn epee() -> Self {
        Objet {
            nom: "Épée".to_string(),
            description: "Une épée simple mais efficace.".to_string(),
            effet: Some(EffetObjet::Arme { degats: De { faces: 8 } }),
        }
    }

    pub fn potion_de_soin() -> Self {
        Objet {
            nom: "Potion de soin".to_string(),
            description: "Un liquide rougeâtre qui referme les blessures.".to_string(),
            effet: Some(EffetObjet::Consommable { soin: 10 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::De;

    #[test]
    fn epee_standard_infflige_un_d8() {
        let epee = Objet::epee();
        assert_eq!(epee.effet, Some(EffetObjet::Arme { degats: De { faces: 8 } }));
    }

    #[test]
    fn potion_de_soin_soigne_dix_points() {
        let potion = Objet::potion_de_soin();
        assert_eq!(potion.effet, Some(EffetObjet::Consommable { soin: 10 }));
    }
}
