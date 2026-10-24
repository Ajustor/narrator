use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use domain::EtatPartie;
use std::fs;
use std::path::PathBuf;

pub struct SauvegardeFichier {
    chemin: PathBuf,
}

impl SauvegardeFichier {
    pub fn nouveau(chemin: PathBuf) -> Self {
        Self { chemin }
    }
}

impl Sauvegarde for SauvegardeFichier {
    fn charger(&self) -> Result<EtatPartie, ErreurSauvegarde> {
        if !self.chemin.exists() {
            return Err(ErreurSauvegarde::FichierAbsent);
        }
        let contenu = fs::read_to_string(&self.chemin).map_err(|e| ErreurSauvegarde::Lecture(e.to_string()))?;
        serde_json::from_str(&contenu).map_err(|e| ErreurSauvegarde::Lecture(e.to_string()))
    }

    fn sauvegarder(&self, etat: &EtatPartie) -> Result<(), ErreurSauvegarde> {
        let contenu = serde_json::to_string_pretty(etat).map_err(|e| ErreurSauvegarde::Ecriture(e.to_string()))?;
        fs::write(&self.chemin, contenu).map_err(|e| ErreurSauvegarde::Ecriture(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Personnage};

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("narrateur_test_{}_{}.json", std::process::id(), nom))
    }

    fn etat_test() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
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
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
        }
    }

    #[test]
    fn charger_sans_fichier_retourne_fichier_absent() {
        let chemin = chemin_temporaire("absent");
        let sauvegarde = SauvegardeFichier::nouveau(chemin);
        assert!(matches!(sauvegarde.charger(), Err(ErreurSauvegarde::FichierAbsent)));
    }

    #[test]
    fn sauvegarder_puis_charger_fait_un_aller_retour() {
        let chemin = chemin_temporaire("roundtrip");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let etat = etat_test();

        sauvegarde.sauvegarder(&etat).expect("sauvegarde échouée");
        let relu = sauvegarde.charger().expect("chargement échoué");

        assert_eq!(relu.personnage.nom, etat.personnage.nom);
        assert_eq!(relu.lieu_actuel, etat.lieu_actuel);

        let _ = fs::remove_file(chemin);
    }

    #[test]
    fn sauvegarder_puis_charger_preserve_un_combat_en_cours() {
        let chemin = chemin_temporaire("combat_roundtrip");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());

        let mut etat = etat_test();
        let mut gobelin = Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3);
        gobelin.pv_actuels = 3;
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![gobelin],
            ennemi_agit_avant_joueur: true,
            tour_actuel: 2,
            termine: false,
            vainqueur: None,
        });

        sauvegarde.sauvegarder(&etat).expect("sauvegarde échouée");
        let relu = sauvegarde.charger().expect("chargement échoué");

        assert_eq!(relu, etat);

        let _ = fs::remove_file(chemin);
    }
}
