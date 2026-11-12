use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};
use domain::{ActionJoueur, EffetObjet, Genre, Personnage, PremisseHistoire};

pub struct FauxNarrateur;

impl Narrateur for FauxNarrateur {
    fn proposer_action(
        &mut self,
        contexte: &ContexteTour,
    ) -> Result<ActionJoueur, ErreurNarrateur> {
        let saisie_minuscule = contexte.saisie_joueur.to_lowercase();

        let action = if saisie_minuscule.contains("attaque") || saisie_minuscule.contains("frappe") {
            trouver_premier_ennemi_vivant(contexte)
                .map(|nom| ActionJoueur::Attaquer { cible: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre { texte: contexte.saisie_joueur.clone(), agressif: false })
        } else if saisie_minuscule.contains("potion") || saisie_minuscule.contains("soin") {
            trouver_premiere_potion(contexte)
                .map(|nom| ActionJoueur::UtiliserObjet { objet: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre { texte: contexte.saisie_joueur.clone(), agressif: false })
        } else if saisie_minuscule.contains("fui") || saisie_minuscule.contains("fuite") {
            ActionJoueur::Fuir
        } else {
            ActionJoueur::ActionLibre { texte: contexte.saisie_joueur.clone(), agressif: false }
        };

        Ok(action)
    }

    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let resultat = if contexte.resume_mecanique.is_empty() {
            "Rien de notable ne se produit.".to_string()
        } else {
            contexte.resume_mecanique.join(" ")
        };
        let narration = format!("Tu as tenté quelque chose... {}", resultat);
        for mot in narration.split_inclusive(' ') {
            sur_fragment(mot);
        }
        Ok(narration)
    }

    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
        Ok(format!("[résumé condensé de {} tours]", anciens.len()))
    }

    fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
        Ok(None)
    }

    fn generer_lieu_initial(&mut self, genre: Genre, _premisse_libre: &str) -> Result<String, ErreurNarrateur> {
        Ok(genre.lieu_initial().to_string())
    }

    fn raconter_introduction(
        &mut self,
        personnage: &Personnage,
        lieu: &str,
        premisse: &PremisseHistoire,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let narration = if premisse.description.is_empty() {
            format!(
                "{} arrive à {}, prêt·e à commencer son aventure.",
                personnage.nom, lieu
            )
        } else {
            format!(
                "{} arrive à {}, prêt·e à commencer son aventure : {}",
                personnage.nom, lieu, premisse.description
            )
        };
        for mot in narration.split_inclusive(' ') {
            sur_fragment(mot);
        }
        Ok(narration)
    }
}

fn trouver_premier_ennemi_vivant(contexte: &ContexteTour) -> Option<String> {
    contexte
        .etat
        .combat_en_cours
        .as_ref()?
        .ennemis
        .iter()
        .find(|e| !e.est_mort())
        .map(|e| e.nom.clone())
}

fn trouver_premiere_potion(contexte: &ContexteTour) -> Option<String> {
    contexte
        .etat
        .personnage
        .inventaire
        .iter()
        .find(|o| matches!(o.effet, Some(EffetObjet::Consommable { .. })))
        .map(|o| o.nom.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Caracteristiques, Combat, De, Ennemi, EtatPartie, Objet, Personnage};

    fn contexte(saisie: &str, combat: Option<Combat>) -> ContexteTour {
        ContexteTour {
            etat: EtatPartie {
                personnage: Personnage::nouveau(
                    "Elowen".into(),
                    "Guerrier".into(),
                    Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
                    vec![Objet::potion_de_soin()],
                ),
                lieu_actuel: "Clairière".into(),
                combat_en_cours: combat,
                resumes: vec![],
                pnj_connus: vec![],
                premisse: Default::default(),
                transcript: vec![],
            },
            saisie_joueur: saisie.to_string(),
        }
    }

    fn combat_avec_gobelin() -> Combat {
        Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        }
    }

    #[test]
    fn attaque_propose_d_attaquer_le_premier_ennemi_vivant() {
        let ctx = contexte("J'attaque le gobelin", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn potion_propose_d_utiliser_la_potion_en_inventaire() {
        let ctx = contexte("Je bois une potion de soin", None);
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn fuite_propose_de_fuir() {
        let ctx = contexte("Je fuis !", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::Fuir);
    }

    #[test]
    fn texte_sans_mot_cle_devient_une_action_libre() {
        let ctx = contexte("J'observe les environs", None);
        let mut narrateur = FauxNarrateur;
        let action = narrateur.proposer_action(&ctx).unwrap();
        assert_eq!(action, ActionJoueur::ActionLibre { texte: "J'observe les environs".into(), agressif: false });
    }

    #[test]
    fn la_narration_du_resultat_est_streamee_fragment_par_fragment() {
        let ctx = contexte("peu importe", None);
        let ctx_resultat = ContexteResultat {
            etat: ctx.etat,
            action: ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            resume_mecanique: vec![],
            saisie_joueur: "J'observe".to_string(),
        };
        let mut narrateur = FauxNarrateur;
        let mut fragments = Vec::new();
        let narration = narrateur
            .raconter_resultat(&ctx_resultat, &mut |frag| fragments.push(frag.to_string()))
            .unwrap();
        assert_eq!(fragments.concat(), narration);
        assert!(fragments.len() > 1);
    }

    #[test]
    fn la_narration_du_resultat_reprend_le_resume_mecanique() {
        let ctx = contexte("peu importe", None);
        let ctx_resultat = ContexteResultat {
            etat: ctx.etat,
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            resume_mecanique: vec!["🎲 Tu attaques Gobelin : touché ! Dégâts : 5".to_string()],
            saisie_joueur: "j'attaque le gobelin".to_string(),
        };
        let mut narrateur = FauxNarrateur;
        let narration = narrateur.raconter_resultat(&ctx_resultat, &mut |_| {}).unwrap();
        assert!(narration.contains("touché"));
    }

    #[test]
    fn la_condensation_resume_le_nombre_de_tours_fournis() {
        let mut narrateur = FauxNarrateur;
        let condense = narrateur
            .condenser_resumes(&["Tour 1.".to_string(), "Tour 2.".to_string()])
            .unwrap();
        assert!(condense.contains('2'));
    }

    #[test]
    fn la_detection_de_pnj_ne_renvoie_rien_par_defaut() {
        let mut narrateur = FauxNarrateur;
        let resultat = narrateur.detecter_pnj("Peu importe.", &[]).unwrap();
        assert_eq!(resultat, None);
    }

    #[test]
    fn l_introduction_mentionne_la_description_de_la_premisse_si_presente() {
        let personnage = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
            vec![],
        );
        let premisse = domain::PremisseHistoire {
            genre: domain::Genre::Horreur,
            description: "une vieille malédiction".into(),
        };
        let mut narrateur = FauxNarrateur;
        let narration = narrateur.raconter_introduction(&personnage, "Clairière", &premisse, &mut |_| {}).unwrap();
        assert!(narration.contains("une vieille malédiction"));
    }

    #[test]
    fn l_introduction_ne_plante_pas_avec_une_premisse_par_defaut() {
        let personnage = Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques { force: 12, dexterite: 14, constitution: 13, intelligence: 10, sagesse: 10, charisme: 8 },
            vec![],
        );
        let mut narrateur = FauxNarrateur;
        let narration = narrateur
            .raconter_introduction(&personnage, "Clairière", &domain::PremisseHistoire::default(), &mut |_| {})
            .unwrap();
        assert!(narration.contains("Elowen"));
    }
}
