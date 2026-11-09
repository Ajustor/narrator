use crate::ports::generateur_de::GenerateurDe;
use domain::{modificateur, Combat, De, Ennemi, Personnage};

pub fn commencer_combat(
    personnage: &Personnage,
    ennemis: Vec<Ennemi>,
    de: &mut dyn GenerateurDe,
) -> Combat {
    let initiative_joueur = de
        .lancer(De { faces: 20 }, modificateur(personnage.caracteristiques.dexterite))
        .total;
    let initiatives_ennemis: Vec<i32> = ennemis
        .iter()
        .map(|e| de.lancer(De { faces: 20 }, e.bonus_initiative).total)
        .collect();

    Combat {
        ennemis,
        initiative_joueur,
        initiatives_ennemis,
        tour_actuel: 0,
        termine: false,
        vainqueur: None,
        joueur_esquive: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::generateur_de::GenerateurDeSeed;
    use domain::{Acteur, Caracteristiques};

    fn perso_dex_14() -> Personnage {
        Personnage::nouveau(
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
        )
    }

    fn gobelin() -> Ennemi {
        Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)
    }

    #[test]
    fn joueur_plus_rapide_que_l_ennemi() {
        // joueur: 10 + modificateur(14)=2 => total 12
        // gobelin: 5 + bonus_initiative 2 => total 7
        let mut de = GenerateurDeSeed::nouveau(vec![10, 5]);
        let combat = commencer_combat(&perso_dex_14(), vec![gobelin()], &mut de);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Joueur, Acteur::Ennemi(0)]);
        assert_eq!(combat.ennemis.len(), 1);
        assert!(!combat.termine);
    }

    #[test]
    fn ennemi_plus_rapide_que_le_joueur() {
        // joueur: 3 + 2 = 5
        // gobelin: 15 + 2 = 17
        let mut de = GenerateurDeSeed::nouveau(vec![3, 15]);
        let combat = commencer_combat(&perso_dex_14(), vec![gobelin()], &mut de);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Ennemi(0), Acteur::Joueur]);
    }
}
