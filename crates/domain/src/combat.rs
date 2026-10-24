use crate::de::JetDe;
use crate::ennemi::Ennemi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vainqueur {
    Joueur,
    Ennemis,
    Fuite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Combat {
    pub ennemis: Vec<Ennemi>,
    pub ennemi_agit_avant_joueur: bool,
    pub tour_actuel: u32,
    pub termine: bool,
    pub vainqueur: Option<Vainqueur>,
}

impl Combat {
    pub fn tous_ennemis_morts(&self) -> bool {
        self.ennemis.iter().all(|e| e.est_mort())
    }

    pub fn trouver_ennemi_vivant_mut(&mut self, nom: &str) -> Option<&mut Ennemi> {
        self.ennemis.iter_mut().find(|e| e.nom == nom && !e.est_mort())
    }

    pub fn terminer(&mut self, vainqueur: Vainqueur) {
        self.termine = true;
        self.vainqueur = Some(vainqueur);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionAttaque {
    pub touche: bool,
    pub degats: i32,
}

pub fn resoudre_attaque(attaque_totale: i32, defense_cible: i32, jet_degats: JetDe) -> ResolutionAttaque {
    let touche = attaque_totale >= defense_cible;
    let degats = if touche { jet_degats.total.max(0) } else { 0 };
    ResolutionAttaque { touche, degats }
}

pub fn determiner_ordre_initiative(initiative_joueur: i32, initiatives_ennemis: &[i32]) -> bool {
    initiatives_ennemis.iter().any(|&i| i > initiative_joueur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::{De, JetDe};
    use crate::ennemi::Ennemi;

    fn gobelin() -> Ennemi {
        Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)
    }

    #[test]
    fn resoudre_attaque_touche_si_total_superieur_ou_egal_a_la_defense() {
        let resolution = resoudre_attaque(12, 12, JetDe::nouveau(5, 0));
        assert!(resolution.touche);
        assert_eq!(resolution.degats, 5);
    }

    #[test]
    fn resoudre_attaque_rate_si_total_inferieur_a_la_defense() {
        let resolution = resoudre_attaque(11, 12, JetDe::nouveau(5, 0));
        assert!(!resolution.touche);
        assert_eq!(resolution.degats, 0);
    }

    #[test]
    fn determiner_ordre_initiative_vrai_si_un_ennemi_est_plus_rapide() {
        assert!(determiner_ordre_initiative(10, &[15]));
        assert!(!determiner_ordre_initiative(15, &[10]));
        assert!(!determiner_ordre_initiative(10, &[10]));
    }

    #[test]
    fn tous_ennemis_morts_vrai_quand_pv_a_zero() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        assert!(!combat.tous_ennemis_morts());
        combat.ennemis[0].subir_degats(100);
        assert!(combat.tous_ennemis_morts());
    }

    #[test]
    fn trouver_ennemi_vivant_mut_ignore_les_morts() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        combat.ennemis[0].subir_degats(100);
        assert!(combat.trouver_ennemi_vivant_mut("Gobelin").is_none());
    }

    #[test]
    fn terminer_fixe_le_vainqueur_et_le_flag_termine() {
        let mut combat = Combat {
            ennemis: vec![gobelin()],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        };
        combat.terminer(Vainqueur::Joueur);
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Joueur));
    }
}
