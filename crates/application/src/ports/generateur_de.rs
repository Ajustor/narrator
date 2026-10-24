use domain::{De, JetDe};

pub trait GenerateurDe {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe;
}

#[cfg(test)]
pub struct GenerateurDeSeed {
    resultats: std::collections::VecDeque<u32>,
}

#[cfg(test)]
impl GenerateurDeSeed {
    pub fn nouveau(resultats: Vec<u32>) -> Self {
        Self { resultats: resultats.into() }
    }
}

#[cfg(test)]
impl GenerateurDe for GenerateurDeSeed {
    fn lancer(&mut self, _de: De, modificateur: i32) -> JetDe {
        let resultat = self
            .resultats
            .pop_front()
            .expect("plus de résultats programmés dans GenerateurDeSeed");
        JetDe::nouveau(resultat, modificateur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generateur_de_seed_renvoie_les_resultats_dans_l_ordre() {
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        assert_eq!(de.lancer(De { faces: 20 }, 2).total, 12);
        assert_eq!(de.lancer(De { faces: 6 }, 0).total, 3);
    }

    #[test]
    #[should_panic(expected = "plus de résultats programmés")]
    fn generateur_de_seed_panique_si_epuise() {
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        de.lancer(De { faces: 20 }, 0);
    }
}
