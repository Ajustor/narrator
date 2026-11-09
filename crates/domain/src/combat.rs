use crate::de::JetDe;
use crate::ennemi::Ennemi;
use serde::{Deserialize, Serialize};

/// Bonus de maîtrise d'un personnage de niveau 1, ajouté à son jet
/// d'attaque (jamais aux dégâts) — les ennemis n'en ont pas besoin, leur
/// `bonus_attaque` joue déjà ce rôle en un seul nombre.
pub const BONUS_MAITRISE: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vainqueur {
    Joueur,
    Ennemis,
    Fuite,
}

/// Un élément de l'ordre d'initiative d'un tour — voir `Combat::ordre_de_tour`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acteur {
    Joueur,
    /// Index dans `Combat::ennemis`.
    Ennemi(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combat {
    pub ennemis: Vec<Ennemi>,
    pub initiative_joueur: i32,
    /// Même index que `ennemis`. `#[serde(default)]` : une sauvegarde
    /// antérieure à ce champ se relit avec un vecteur vide, qui ne
    /// correspond plus en longueur à `ennemis` — `ordre_de_tour` détecte ce
    /// cas et retombe sur « le joueur d'abord » plutôt que de paniquer sur
    /// un index hors limites.
    #[serde(default)]
    pub initiatives_ennemis: Vec<i32>,
    pub tour_actuel: u32,
    pub termine: bool,
    pub vainqueur: Option<Vainqueur>,
    /// `true` depuis que le joueur a choisi d'esquiver et jusqu'à ce qu'il
    /// agisse à nouveau : toutes les attaques ennemies rencontrées dans cet
    /// intervalle (y compris, le cas échéant, celles d'un tour suivant si
    /// des ennemis plus rapides agissent avant la prochaine action du
    /// joueur) sont faites avec désavantage. `#[serde(default)]` pour les
    /// sauvegardes antérieures à ce champ.
    #[serde(default)]
    pub joueur_esquive: bool,
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

    /// Ordre de résolution d'un tour complet : chacun (joueur et ennemis)
    /// classé par initiative décroissante, une égalité étant tranchée en
    /// faveur du joueur. Remplace l'ancien booléen global
    /// `ennemi_agit_avant_joueur`, qui regroupait tous les ennemis dans un
    /// seul bloc avant ou après le joueur au lieu de les classer
    /// individuellement.
    pub fn ordre_de_tour(&self) -> Vec<Acteur> {
        if self.initiatives_ennemis.len() != self.ennemis.len() {
            let mut ordre = vec![Acteur::Joueur];
            ordre.extend((0..self.ennemis.len()).map(Acteur::Ennemi));
            return ordre;
        }

        let mut avant: Vec<usize> = (0..self.ennemis.len())
            .filter(|&i| self.initiatives_ennemis[i] > self.initiative_joueur)
            .collect();
        avant.sort_by_key(|&i| std::cmp::Reverse(self.initiatives_ennemis[i]));

        let mut apres: Vec<usize> = (0..self.ennemis.len())
            .filter(|&i| self.initiatives_ennemis[i] <= self.initiative_joueur)
            .collect();
        apres.sort_by_key(|&i| std::cmp::Reverse(self.initiatives_ennemis[i]));

        let mut ordre: Vec<Acteur> = avant.into_iter().map(Acteur::Ennemi).collect();
        ordre.push(Acteur::Joueur);
        ordre.extend(apres.into_iter().map(Acteur::Ennemi));
        ordre
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultatJetAttaque {
    pub touche: bool,
    pub critique: bool,
}

pub fn attaque_touche(attaque_totale: i32, defense_cible: i32) -> bool {
    attaque_totale >= defense_cible
}

/// Évalue un jet d'attaque déjà lancé face à une défense. Un 1 naturel rate
/// toujours, même si le total aurait suffi ; un 20 naturel touche toujours
/// et déclenche un critique, même si le total n'aurait pas suffi. Entre les
/// deux, c'est la défense qui tranche normalement. Lire `resultat` (le jet
/// brut, avant modificateur) et pas `total` est essentiel ici.
pub fn evaluer_jet_attaque(jet_attaque: JetDe, defense_cible: i32) -> ResultatJetAttaque {
    if jet_attaque.resultat == 1 {
        return ResultatJetAttaque { touche: false, critique: false };
    }
    if jet_attaque.resultat == 20 {
        return ResultatJetAttaque { touche: true, critique: true };
    }
    ResultatJetAttaque { touche: attaque_touche(jet_attaque.total, defense_cible), critique: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::de::De;
    use crate::ennemi::Ennemi;

    fn gobelin() -> Ennemi {
        Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)
    }

    fn combat_avec(ennemis: Vec<Ennemi>, initiative_joueur: i32, initiatives_ennemis: Vec<i32>) -> Combat {
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

    #[test]
    fn evaluer_jet_attaque_touche_si_total_superieur_ou_egal_a_la_defense() {
        let resultat = evaluer_jet_attaque(JetDe::nouveau(10, 2), 12);
        assert!(resultat.touche);
        assert!(!resultat.critique);
    }

    #[test]
    fn evaluer_jet_attaque_rate_si_total_inferieur_a_la_defense() {
        let resultat = evaluer_jet_attaque(JetDe::nouveau(5, 2), 12);
        assert!(!resultat.touche);
        assert!(!resultat.critique);
    }

    #[test]
    fn evaluer_jet_attaque_un_20_naturel_touche_et_critique_meme_si_le_total_ne_suffirait_pas() {
        let resultat = evaluer_jet_attaque(JetDe::nouveau(20, -5), 100);
        assert!(resultat.touche);
        assert!(resultat.critique);
    }

    #[test]
    fn evaluer_jet_attaque_un_1_naturel_rate_meme_si_le_total_suffirait() {
        let resultat = evaluer_jet_attaque(JetDe::nouveau(1, 50), 12);
        assert!(!resultat.touche);
        assert!(!resultat.critique);
    }

    #[test]
    fn tous_ennemis_morts_vrai_quand_pv_a_zero() {
        let mut combat = combat_avec(vec![gobelin()], 10, vec![0]);
        assert!(!combat.tous_ennemis_morts());
        combat.ennemis[0].subir_degats(100);
        assert!(combat.tous_ennemis_morts());
    }

    #[test]
    fn trouver_ennemi_vivant_mut_ignore_les_morts() {
        let mut combat = combat_avec(vec![gobelin()], 10, vec![0]);
        combat.ennemis[0].subir_degats(100);
        assert!(combat.trouver_ennemi_vivant_mut("Gobelin").is_none());
    }

    #[test]
    fn terminer_fixe_le_vainqueur_et_le_flag_termine() {
        let mut combat = combat_avec(vec![gobelin()], 10, vec![0]);
        combat.terminer(Vainqueur::Joueur);
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Joueur));
    }

    #[test]
    fn ordre_de_tour_met_le_joueur_en_premier_si_plus_rapide_que_tous_les_ennemis() {
        let combat = combat_avec(vec![gobelin(), gobelin()], 10, vec![5, 3]);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Joueur, Acteur::Ennemi(0), Acteur::Ennemi(1)]);
    }

    #[test]
    fn ordre_de_tour_met_un_ennemi_plus_rapide_avant_le_joueur() {
        let combat = combat_avec(vec![gobelin()], 10, vec![15]);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Ennemi(0), Acteur::Joueur]);
    }

    #[test]
    fn ordre_de_tour_entrelace_les_ennemis_plus_rapides_et_plus_lents_que_le_joueur() {
        // ennemi 0 (18) plus rapide que le joueur (10), ennemi 1 (15) aussi
        // mais plus lent que l'ennemi 0, ennemi 2 (3) plus lent que le joueur.
        let combat = combat_avec(vec![gobelin(), gobelin(), gobelin()], 10, vec![18, 15, 3]);
        assert_eq!(
            combat.ordre_de_tour(),
            vec![Acteur::Ennemi(0), Acteur::Ennemi(1), Acteur::Joueur, Acteur::Ennemi(2)]
        );
    }

    #[test]
    fn ordre_de_tour_tranche_une_egalite_en_faveur_du_joueur() {
        let combat = combat_avec(vec![gobelin()], 10, vec![10]);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Joueur, Acteur::Ennemi(0)]);
    }

    #[test]
    fn ordre_de_tour_retombe_sur_joueur_d_abord_si_les_longueurs_ne_correspondent_plus() {
        // Simule une sauvegarde antérieure au champ `initiatives_ennemis`,
        // relue avec un vecteur vide alors que `ennemis` en contient un.
        let combat = combat_avec(vec![gobelin()], 10, vec![]);
        assert_eq!(combat.ordre_de_tour(), vec![Acteur::Joueur, Acteur::Ennemi(0)]);
    }
}
