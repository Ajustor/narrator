use application::ports::narrateur::{ContexteTour, ErreurNarrateur, Narrateur, ReponseMj};
use domain::{ActionJoueur, EffetObjet};

pub struct FauxNarrateur;

impl Narrateur for FauxNarrateur {
    fn raconter(
        &mut self,
        contexte: &ContexteTour,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ReponseMj, ErreurNarrateur> {
        let saisie_minuscule = contexte.saisie_joueur.to_lowercase();

        let action = if saisie_minuscule.contains("attaque") || saisie_minuscule.contains("frappe") {
            trouver_premier_ennemi_vivant(contexte)
                .map(|nom| ActionJoueur::Attaquer { cible: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre(contexte.saisie_joueur.clone()))
        } else if saisie_minuscule.contains("potion") || saisie_minuscule.contains("soin") {
            trouver_premiere_potion(contexte)
                .map(|nom| ActionJoueur::UtiliserObjet { objet: nom })
                .unwrap_or_else(|| ActionJoueur::ActionLibre(contexte.saisie_joueur.clone()))
        } else if saisie_minuscule.contains("fui") || saisie_minuscule.contains("fuite") {
            ActionJoueur::Fuir
        } else {
            ActionJoueur::ActionLibre(contexte.saisie_joueur.clone())
        };

        let narration = format!("Tu tentes : \"{}\"...", contexte.saisie_joueur);
        for mot in narration.split_inclusive(' ') {
            sur_fragment(mot);
        }

        Ok(ReponseMj { narration, action_proposee: action })
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
            },
            saisie_joueur: saisie.to_string(),
        }
    }

    fn combat_avec_gobelin() -> Combat {
        Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            ennemi_agit_avant_joueur: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        }
    }

    #[test]
    fn attaque_propose_d_attaquer_le_premier_ennemi_vivant() {
        let ctx = contexte("J'attaque le gobelin", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn potion_propose_d_utiliser_la_potion_en_inventaire() {
        let ctx = contexte("Je bois une potion de soin", None);
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() });
    }

    #[test]
    fn fuite_propose_de_fuir() {
        let ctx = contexte("Je fuis !", Some(combat_avec_gobelin()));
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::Fuir);
    }

    #[test]
    fn texte_sans_mot_cle_devient_une_action_libre() {
        let ctx = contexte("J'observe les environs", None);
        let mut narrateur = FauxNarrateur;
        let reponse = narrateur.raconter(&ctx, &mut |_| {}).unwrap();
        assert_eq!(reponse.action_proposee, ActionJoueur::ActionLibre("J'observe les environs".into()));
    }

    #[test]
    fn la_narration_est_streamee_fragment_par_fragment() {
        let ctx = contexte("J'observe", None);
        let mut narrateur = FauxNarrateur;
        let mut fragments = Vec::new();
        let reponse = narrateur.raconter(&ctx, &mut |frag| fragments.push(frag.to_string())).unwrap();
        assert_eq!(fragments.concat(), reponse.narration);
        assert!(fragments.len() > 1);
    }
}
