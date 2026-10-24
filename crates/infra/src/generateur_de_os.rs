use application::ports::generateur_de::GenerateurDe;
use domain::{De, JetDe};
use rand::Rng;

pub struct GenerateurDeOs;

impl GenerateurDe for GenerateurDeOs {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe {
        let resultat = rand::thread_rng().gen_range(1..=de.faces);
        JetDe::nouveau(resultat, modificateur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lancer_reste_dans_les_bornes_du_de() {
        let mut generateur = GenerateurDeOs;
        for _ in 0..1000 {
            let jet = generateur.lancer(De { faces: 6 }, 0);
            assert!(jet.resultat >= 1 && jet.resultat <= 6, "résultat hors bornes: {}", jet.resultat);
        }
    }

    #[test]
    fn lancer_applique_le_modificateur_au_total() {
        let mut generateur = GenerateurDeOs;
        let jet = generateur.lancer(De { faces: 20 }, 5);
        assert_eq!(jet.total, jet.resultat as i32 + 5);
    }
}
