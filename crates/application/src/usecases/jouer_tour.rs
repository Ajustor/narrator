use crate::ports::generateur_de::GenerateurDe;
use crate::ports::narrateur::{ContexteTour, Narrateur, ReponseMj};
use domain::{resoudre_attaque, ActionJoueur, De, EffetObjet, EtatPartie, Vainqueur};

#[derive(Debug, PartialEq)]
pub enum ErreurTour {
    AucunCombatEnCours,
    CibleInvalide,
    ObjetInvalide,
    EchecNarrateur,
}

pub fn valider_action(action: &ActionJoueur, etat: &EtatPartie) -> Result<(), ErreurTour> {
    match action {
        ActionJoueur::Attaquer { cible } => {
            let combat = etat.combat_en_cours.as_ref().ok_or(ErreurTour::AucunCombatEnCours)?;
            let trouve = combat.ennemis.iter().any(|e| &e.nom == cible && !e.est_mort());
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::CibleInvalide)
            }
        }
        ActionJoueur::UtiliserObjet { objet } => {
            let trouve = etat.personnage.inventaire.iter().any(|o| {
                &o.nom == objet && matches!(o.effet, Some(EffetObjet::Consommable { .. }))
            });
            if trouve {
                Ok(())
            } else {
                Err(ErreurTour::ObjetInvalide)
            }
        }
        ActionJoueur::Fuir => {
            if etat.combat_en_cours.is_some() {
                Ok(())
            } else {
                Err(ErreurTour::AucunCombatEnCours)
            }
        }
        ActionJoueur::ActionLibre(_) => Ok(()),
    }
}

pub fn resoudre_action_joueur(
    etat: &mut EtatPartie,
    action: &ActionJoueur,
    de: &mut dyn GenerateurDe,
) -> Vec<String> {
    let mut resume = Vec::new();
    match action {
        ActionJoueur::Attaquer { cible } => {
            let modificateur_dex = domain::modificateur(etat.personnage.caracteristiques.dexterite);
            let jet_attaque = de.lancer(De { faces: 20 }, modificateur_dex);
            let arme = etat.personnage.arme_equipee();
            let jet_degats = de.lancer(arme, 0);

            if let Some(combat) = etat.combat_en_cours.as_mut() {
                if let Some(ennemi) = combat.trouver_ennemi_vivant_mut(cible) {
                    let resolution = resoudre_attaque(jet_attaque.total, ennemi.defense, jet_degats);
                    if resolution.touche {
                        ennemi.subir_degats(resolution.degats);
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — touché ! Dégâts : {}",
                            ennemi.nom, jet_attaque.total, ennemi.defense, resolution.degats
                        ));
                    } else {
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — manqué",
                            ennemi.nom, jet_attaque.total, ennemi.defense
                        ));
                    }
                }
            }
        }
        ActionJoueur::UtiliserObjet { objet } => {
            if let Some(obj) = etat.personnage.retirer_objet(objet) {
                if let Some(EffetObjet::Consommable { soin }) = obj.effet {
                    etat.personnage.soigner(soin as i32);
                    resume.push(format!("🎲 Tu utilises {} : +{} PV", obj.nom, soin));
                }
            }
        }
        ActionJoueur::Fuir => {
            if let Some(combat) = etat.combat_en_cours.as_mut() {
                combat.terminer(Vainqueur::Fuite);
            }
            resume.push("Tu prends la fuite.".to_string());
        }
        ActionJoueur::ActionLibre(_) => {}
    }
    resume
}

pub struct ResultatTour {
    pub narration: String,
    pub resume_mecanique: Vec<String>,
    pub combat_termine: bool,
}

pub fn resoudre_action_ennemis(etat: &mut EtatPartie, de: &mut dyn GenerateurDe) -> Vec<String> {
    let mut resume = Vec::new();
    let defense_joueur = etat.personnage.defense();

    let Some(combat) = etat.combat_en_cours.as_mut() else {
        return resume;
    };
    if combat.termine {
        return resume;
    }

    let mut degats_totaux = 0;
    for ennemi in combat.ennemis.iter_mut().filter(|e| !e.est_mort()) {
        let jet_attaque = de.lancer(De { faces: 20 }, ennemi.bonus_attaque);
        let jet_degats = de.lancer(ennemi.degats, 0);
        let resolution = resoudre_attaque(jet_attaque.total, defense_joueur, jet_degats);
        if resolution.touche {
            degats_totaux += resolution.degats;
            resume.push(format!(
                "🎲 {} t'attaque : {} vs défense {} — touché ! Dégâts : {}",
                ennemi.nom, jet_attaque.total, defense_joueur, resolution.degats
            ));
        } else {
            resume.push(format!(
                "🎲 {} t'attaque : {} vs défense {} — manqué",
                ennemi.nom, jet_attaque.total, defense_joueur
            ));
        }
    }

    etat.personnage.subir_degats(degats_totaux);
    resume
}

fn combat_termine(etat: &EtatPartie) -> bool {
    etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(true)
}

fn terminer_si_ennemis_morts(etat: &mut EtatPartie) {
    if let Some(combat) = etat.combat_en_cours.as_mut() {
        if combat.tous_ennemis_morts() {
            combat.terminer(Vainqueur::Joueur);
        }
    }
}

fn terminer_si_joueur_mort(etat: &mut EtatPartie) {
    if etat.personnage.est_mort() {
        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.terminer(Vainqueur::Ennemis);
        }
    }
}

pub struct JouerTour;

impl JouerTour {
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        let contexte = ContexteTour {
            etat: etat.clone(),
            saisie_joueur: saisie.to_string(),
        };
        let ReponseMj { narration, action_proposee } = narrateur
            .raconter(&contexte, sur_fragment)
            .map_err(|_| ErreurTour::EchecNarrateur)?;

        valider_action(&action_proposee, etat)?;

        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.tour_actuel += 1;
        }

        let mut resume = Vec::new();
        let fuite = matches!(action_proposee, ActionJoueur::Fuir);

        if fuite {
            resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
        } else {
            let ennemi_avant = etat
                .combat_en_cours
                .as_ref()
                .map(|c| c.ennemi_agit_avant_joueur)
                .unwrap_or(false);

            if ennemi_avant {
                resume.extend(resoudre_action_ennemis(etat, de));
                terminer_si_joueur_mort(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
                    terminer_si_ennemis_morts(etat);
                }
            } else {
                resume.extend(resoudre_action_joueur(etat, &action_proposee, de));
                terminer_si_ennemis_morts(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_ennemis(etat, de));
                    terminer_si_joueur_mort(etat);
                }
            }
        }

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            combat_termine: etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, Objet, Personnage};

    fn etat_sans_combat() -> EtatPartie {
        EtatPartie {
            personnage: Personnage::nouveau(
                "Elowen".into(),
                "Guerrier".into(),
                Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                vec![Objet::potion_de_soin()],
            ),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
        }
    }

    fn etat_en_combat() -> EtatPartie {
        let mut etat = etat_sans_combat();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        etat
    }

    #[test]
    fn attaquer_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn attaquer_une_cible_inexistante_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Dragon".into() }, &etat_en_combat());
        assert_eq!(resultat, Err(ErreurTour::CibleInvalide));
    }

    #[test]
    fn attaquer_une_cible_vivante_est_valide() {
        let resultat = valider_action(&ActionJoueur::Attaquer { cible: "Gobelin".into() }, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn utiliser_un_objet_absent_est_invalide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion inexistante".into() }, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_arme_via_utiliser_objet_est_invalide() {
        let mut etat = etat_sans_combat();
        etat.personnage.inventaire.push(Objet::epee());
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Épée".into() }, &etat);
        assert_eq!(resultat, Err(ErreurTour::ObjetInvalide));
    }

    #[test]
    fn utiliser_une_potion_presente_est_valide() {
        let resultat = valider_action(&ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn fuir_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn fuir_en_combat_est_valide() {
        let resultat = valider_action(&ActionJoueur::Fuir, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn action_libre_est_toujours_valide() {
        let resultat = valider_action(&ActionJoueur::ActionLibre("Je regarde autour de moi".into()), &etat_sans_combat());
        assert_eq!(resultat, Ok(()));
    }

    use crate::ports::generateur_de::GenerateurDeSeed;
    use domain::Vainqueur;

    #[test]
    fn attaquer_qui_touche_infflige_des_degats() {
        let mut etat = etat_en_combat();
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché)
        // dégâts: d8=5 (épée par défaut quand aucune arme en inventaire -> mains nues d4 ici, voir note)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 4); // 7 - 3
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn attaquer_qui_rate_n_infflige_aucun_degat() {
        let mut etat = etat_en_combat();
        // attaque: d20=5 + mod_dex(14)=2 => 7 < defense 12 (manqué)
        let mut de = GenerateurDeSeed::nouveau(vec![5, 3]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 7);
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn utiliser_objet_soigne_et_retire_l_objet_de_l_inventaire() {
        let mut etat = etat_sans_combat();
        etat.personnage.subir_degats(5);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &mut de);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max);
        assert!(etat.personnage.inventaire.is_empty());
        assert!(resume[0].contains("Potion de soin"));
    }

    #[test]
    fn fuir_termine_le_combat_avec_le_vainqueur_fuite() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        resoudre_action_joueur(&mut etat, &ActionJoueur::Fuir, &mut de);
        let combat = etat.combat_en_cours.as_ref().unwrap();
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Fuite));
    }

    #[test]
    fn action_libre_ne_consomme_aucun_de() {
        let mut etat = etat_sans_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::ActionLibre("J'observe".into()), &mut de);
        assert!(resume.is_empty());
    }

    use crate::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};

    struct NarrateurDeTest {
        action: ActionJoueur,
    }

    impl Narrateur for NarrateurDeTest {
        fn raconter(
            &mut self,
            _contexte: &ContexteTour,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<ReponseMj, ErreurNarrateur> {
            sur_fragment("narration de test");
            Ok(ReponseMj {
                narration: "narration de test".to_string(),
                action_proposee: self.action.clone(),
            })
        }
    }

    #[test]
    fn resoudre_action_ennemis_infflige_des_degats_au_joueur() {
        let mut etat = etat_en_combat();
        // ennemi (Gobelin, bonus_attaque 3) : d20=10 => total 13 >= défense joueur 12 (touché)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 4]); // attaque puis dégâts (d6=4)
        let resume = resoudre_action_ennemis(&mut etat, &mut de);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn resoudre_action_ennemis_ne_fait_rien_si_combat_termine() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().terminer(Vainqueur::Joueur);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_ennemis(&mut etat, &mut de);
        assert!(resume.is_empty());
    }

    #[test]
    fn jouer_tour_joueur_tue_l_ennemi_et_termine_le_combat() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemis[0].pv_actuels = 1;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3 (mains nues, tue le gobelin à 1 pv)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert!(resultat.combat_termine);
        assert_eq!(etat.combat_en_cours.as_ref().unwrap().vainqueur, Some(Vainqueur::Joueur));
    }

    #[test]
    fn jouer_tour_ennemi_plus_rapide_agit_avant_le_joueur() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemi_agit_avant_joueur = true;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        // ennemi: d20=10 + bonus_attaque 3 = 13 >= defense joueur 12 (touché) ; dégâts d6=4
        let mut de = GenerateurDeSeed::nouveau(vec![10, 4]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert!(!resultat.combat_termine);
    }

    #[test]
    fn jouer_tour_hors_combat_ne_signale_jamais_combat_termine() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de, &mut |_| {}).unwrap();
        assert!(!resultat.combat_termine);
    }

    #[test]
    fn jouer_tour_rejette_une_action_invalide_sans_consommer_de_de() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque", &mut narrateur, &mut de, &mut |_| {});
        assert!(matches!(resultat, Err(ErreurTour::AucunCombatEnCours)));
    }
}
