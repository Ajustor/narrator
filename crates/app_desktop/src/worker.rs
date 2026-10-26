use application::ports::generateur_de::GenerateurDe;
use application::ports::narrateur::Narrateur;
use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use application::usecases::commencer_combat::commencer_combat;
use application::usecases::creer_personnage::creer_personnage;
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, Ennemi, EtatPartie};
use std::path::Path;

use crate::protocole::{MessageVersUi, MessageVersWorker};

pub fn traiter_message(
    etat: &mut Option<EtatPartie>,
    message: MessageVersWorker,
    narrateur: &mut dyn Narrateur,
    de: &mut dyn GenerateurDe,
    sauvegarde: &dyn Sauvegarde,
    chemin_sauvegarde: &Path,
    sur_message_ui: &mut dyn FnMut(MessageVersUi),
) {
    match message {
        MessageVersWorker::CreerPersonnage { nom, classe, valeurs } => {
            let personnage = match creer_personnage(&nom, &classe, valeurs) {
                Ok(personnage) => personnage,
                Err(_) => return,
            };
            let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
            let combat = commencer_combat(&personnage, vec![gobelin], de);
            let nouvel_etat = EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: Some(combat),
                resumes: Vec::new(),
                pnj_connus: Vec::new(),
            };
            *etat = Some(nouvel_etat.clone());
            sur_message_ui(MessageVersUi::PartieCreee(nouvel_etat));
        }

        MessageVersWorker::JouerTour(saisie) => {
            let Some(etat_courant) = etat.as_mut() else { return };
            let resultat = JouerTour::executer(etat_courant, &saisie, narrateur, de, &mut |fragment: &str| {
                sur_message_ui(MessageVersUi::NarrationFragment(fragment.to_string()));
            });

            match resultat {
                Ok(resultat) => {
                    if resultat.combat_termine {
                        let _ = std::fs::remove_file(chemin_sauvegarde);
                    } else {
                        let _ = sauvegarder_partie(sauvegarde, etat_courant);
                    }
                    sur_message_ui(MessageVersUi::TourTermine {
                        etat: etat_courant.clone(),
                        resultat_narration: resultat.narration,
                        resume_mecanique: resultat.resume_mecanique,
                        combat_termine: resultat.combat_termine,
                    });
                }
                Err(erreur) => {
                    sur_message_ui(MessageVersUi::TourEchoue(format!("{:?}", erreur)));
                }
            }
        }

        MessageVersWorker::NouvellePartie => {
            let _ = std::fs::remove_file(chemin_sauvegarde);
            *etat = None;
            sur_message_ui(MessageVersUi::PartieReinitialisee);
        }
    }
}

pub fn charger_etat_initial(sauvegarde: &dyn Sauvegarde) -> Option<EtatPartie> {
    match charger_partie(sauvegarde) {
        Ok(etat) => Some(etat),
        Err(ErreurSauvegarde::FichierAbsent) => None,
        Err(_) => None,
    }
}

#[allow(dead_code)]
pub fn lancer(
    rx: std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    chemin_modele: std::path::PathBuf,
    chemin_sauvegarde: std::path::PathBuf,
) {
    let mut narrateur = match infra::LlamaNarrateur::charger(&chemin_modele) {
        Ok(narrateur) => narrateur,
        Err(erreur) => {
            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                "échec du chargement du modèle : {erreur}"
            )));
            return;
        }
    };
    let mut de = infra::GenerateurDeOs;
    let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde.clone());

    let mut etat = charger_etat_initial(&sauvegarde);
    let _ = tx.unbounded_send(MessageVersUi::EtatInitial(etat.clone()));

    while let Ok(message) = rx.recv() {
        traiter_message(
            &mut etat,
            message,
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin_sauvegarde,
            &mut |msg| {
                let _ = tx.unbounded_send(msg);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::FauxNarrateur;
    use infra::SauvegardeFichier;
    use std::path::PathBuf;

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("app_desktop_test_{}_{}.json", std::process::id(), nom))
    }

    struct GenerateurDeFixe;

    impl GenerateurDe for GenerateurDeFixe {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            domain::JetDe::nouveau(1, modificateur)
        }
    }

    #[test]
    fn creer_personnage_envoie_partie_creee() {
        let chemin = chemin_temporaire("creer");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = None;
        let mut narrateur = FauxNarrateur;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::CreerPersonnage {
                nom: "Elowen".to_string(),
                classe: "Guerrier".to_string(),
                valeurs: [15, 14, 13, 12, 10, 8],
            },
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(etat.is_some());
        assert_eq!(etat.as_ref().unwrap().personnage.nom, "Elowen");
        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::PartieCreee(_))));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurTourReussi;

    impl Narrateur for NarrateurTourReussi {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok("Il ne se passe rien de notable.".to_string())
        }

        fn condenser_resumes(
            &mut self,
            _anciens: &[String],
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(
            &mut self,
            _narration: &str,
            _noms_connus: &[String],
        ) -> Result<Option<application::ports::narrateur::PnjDetecte>, application::ports::narrateur::ErreurNarrateur> {
            Ok(None)
        }
    }

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: domain::Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                domain::Caracteristiques {
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
            pnj_connus: vec![],
        }
    }

    #[test]
    fn jouer_tour_hors_combat_envoie_fragments_puis_tour_termine() {
        let chemin = chemin_temporaire("tour_reussi");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourReussi;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { combat_termine: false, .. })));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurTourEnEchec;

    impl Narrateur for NarrateurTourEnEchec {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::Attaquer { cible: "Gobelin".into() })
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn condenser_resumes(
            &mut self,
            _anciens: &[String],
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(
            &mut self,
            _narration: &str,
            _noms_connus: &[String],
        ) -> Result<Option<application::ports::narrateur::PnjDetecte>, application::ports::narrateur::ErreurNarrateur> {
            Ok(None)
        }
    }

    #[test]
    fn attaquer_sans_combat_envoie_tour_echoue() {
        let chemin = chemin_temporaire("tour_echec");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourEnEchec;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'attaque".to_string()),
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourEchoue(_))));

        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn nouvelle_partie_reinitialise_l_etat_et_supprime_la_sauvegarde() {
        let chemin = chemin_temporaire("nouvelle_partie");
        std::fs::write(&chemin, "contenu factice").expect("écriture échouée");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = FauxNarrateur;
        let mut de = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();

        traiter_message(
            &mut etat,
            MessageVersWorker::NouvellePartie,
            &mut narrateur,
            &mut de,
            &sauvegarde,
            &chemin,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(etat.is_none());
        assert!(!chemin.exists());
        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::PartieReinitialisee)));
    }
}
