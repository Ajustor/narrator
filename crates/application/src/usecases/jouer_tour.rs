use crate::ports::generateur_de::GenerateurDe;
use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
use crate::usecases::commencer_combat::commencer_combat;
use domain::{resoudre_attaque, ActionJoueur, De, EffetObjet, Ennemi, EtatPartie, FichePnj, Vainqueur};

// SEUIL_MOTS_RESUMES est choisi pour rester largement sous TAILLE_CONTEXTE
// (4096 tokens, dans infra::llama_narrateur) une fois converti en tokens —
// si l'un de ces deux nombres change, vérifier que l'autre le permet encore.
pub const SEUIL_MOTS_RESUMES: usize = 800;
pub const NB_RESUMES_RECENTS_CONSERVES: usize = 5;
const LIMITE_DURE_RESUMES: usize = 4 * NB_RESUMES_RECENTS_CONSERVES;

fn message_echec_narrateur(erreur: ErreurNarrateur) -> ErreurTour {
    let ErreurNarrateur::EchecGeneration(message) = erreur;
    ErreurTour::EchecNarrateur(message)
}

#[derive(Debug, PartialEq)]
pub enum ErreurTour {
    AucunCombatEnCours,
    CibleInvalide,
    ObjetInvalide,
    EchecNarrateur(String),
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

            if let Some(combat) = etat.combat_en_cours.as_mut() {
                if let Some(ennemi) = combat.trouver_ennemi_vivant_mut(cible) {
                    // Le jet de dégâts n'est lancé que si l'attaque touche
                    // réellement : avec un dé automatique ça ne changeait
                    // rien (lancé dans tous les cas puis ignoré en cas de
                    // raté), mais avec le dé du joueur (voir `de`, qui peut
                    // maintenant attendre une action du joueur), lui
                    // demander un jet de dégâts pour une attaque déjà
                    // ratée n'aurait aucun sens.
                    if domain::attaque_touche(jet_attaque.total, ennemi.defense) {
                        let jet_degats = de.lancer(arme, 0);
                        let resolution = resoudre_attaque(jet_attaque.total, ennemi.defense, jet_degats);
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
    /// `Some(vainqueur)` si un combat s'est conclu *pendant ce tour* —
    /// quel que soit le vainqueur. Sert à afficher un message transitoire
    /// ("Victoire !", "Tu as fui.", ...) ; ne signifie pas que la partie
    /// est terminée, voir `partie_terminee`.
    pub vainqueur_combat: Option<Vainqueur>,
    /// `true` seulement si le joueur est mort (`vainqueur_combat ==
    /// Some(Vainqueur::Ennemis)`) : seul cas qui termine réellement la
    /// partie. Une victoire ou une fuite réussie remet `combat_en_cours`
    /// à `None` et laisse la partie continuer en exploration.
    pub partie_terminee: bool,
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

/// Déclenche la toute première rencontre d'une histoire (le Gobelin de la
/// clairière) si elle ne l'a pas encore été — appelé en tout début de
/// `JouerTour::executer`, avant même de classifier l'action du joueur.
///
/// Sans ça, le combat devrait être lancé par l'appelant dès la création du
/// personnage (comportement précédent) : le panneau de combat apparaissait
/// alors instantanément, avant même que le joueur ait fait quoi que ce
/// soit, en même temps que la scène d'ouverture. En le déclenchant ici à
/// la place, au tout premier tour réel, le joueur voit d'abord
/// l'exploration pure (l'intro) puis entre en combat seulement quand il
/// agit — et comme `etat` (donc `combat_en_cours`) est mis à jour avant de
/// construire le `ContexteTour` de classification, le narrateur voit déjà
/// le Gobelin et peut correctement classifier un premier message du type
/// "j'attaque" en `Attaquer`, sans quoi `valider_action` le rejetterait
/// (pas de combat en cours à ce moment-là).
fn amorcer_rencontre_initiale(etat: &mut EtatPartie, de_monde: &mut dyn GenerateurDe, resume: &mut Vec<String>) {
    if etat.rencontre_initiale_declenchee {
        return;
    }
    let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
    let combat = commencer_combat(&etat.personnage, vec![gobelin], de_monde);
    etat.combat_en_cours = Some(combat);
    etat.rencontre_initiale_declenchee = true;
    resume.push("⚔️ Un Gobelin surgit et t'attaque !".to_string());
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

// Note : le résumé condensé revient en tête de liste et sera donc lui-même
// inclus dans `anciens` lors d'un futur cycle de condensation — l'histoire
// la plus ancienne est donc recondensée de façon cumulative (perte
// d'information progressive), ce qui est acceptable pour une partie solo.
fn condenser_si_necessaire(etat: &mut EtatPartie, narrateur: &mut dyn Narrateur) {
    let total_mots: usize = etat.resumes.iter().map(|r| r.split_whitespace().count()).sum();
    if total_mots <= SEUIL_MOTS_RESUMES || etat.resumes.len() <= NB_RESUMES_RECENTS_CONSERVES {
        return;
    }

    let point_de_coupe = etat.resumes.len() - NB_RESUMES_RECENTS_CONSERVES;
    let anciens = &etat.resumes[..point_de_coupe];
    if let Ok(condense) = narrateur.condenser_resumes(anciens) {
        if !condense.trim().is_empty() {
            let mut nouveaux = vec![condense];
            nouveaux.extend_from_slice(&etat.resumes[point_de_coupe..]);
            etat.resumes = nouveaux;
            return;
        }
    }

    // La condensation a échoué, ou a renvoyé un résultat vide : si les
    // résumés ont malgré tout fini par dépasser une limite dure (échecs
    // répétés), on tronque directement, sans IA, pour borner la taille de
    // la sauvegarde plutôt que de la laisser grossir indéfiniment.
    if etat.resumes.len() > LIMITE_DURE_RESUMES {
        let point_de_coupe_dur = etat.resumes.len() - NB_RESUMES_RECENTS_CONSERVES;
        etat.resumes.drain(..point_de_coupe_dur);
    }
}

// Un modèle quantifié de petite taille (ex : Qwen2.5-3B en Q4_K_M) ne
// fournit pas toujours une description dans sa réponse JSON de détection,
// même avec les exemples explicites du prompt (`construire_prompt_detection_pnj`
// dans `infra`, qui a d'abord corrigé le problème plus grave où le modèle
// employait un nom commun comme "le tavernier" au lieu d'inventer un nom
// propre). Une fiche créée sans description se complète donc elle-même dès
// qu'une détection ultérieure en fournit une — voir la vérification
// `description.is_empty()` ci-dessous — plutôt que de rester vide pour
// toujours.
fn enregistrer_pnj_si_pertinent(etat: &mut EtatPartie, narration: &str, narrateur: &mut dyn Narrateur) {
    if !combat_termine(etat) {
        return;
    }

    let noms_connus: Vec<String> = etat.pnj_connus.iter().map(|p| p.nom.clone()).collect();
    if let Ok(Some(detecte)) = narrateur.detecter_pnj(narration, &noms_connus) {
        let fiche_existante = etat
            .pnj_connus
            .iter_mut()
            .find(|p| p.nom.eq_ignore_ascii_case(&detecte.nom));
        if let Some(fiche) = fiche_existante {
            if fiche.description.is_empty() {
                if let Some(description) = detecte.nouvelle_description {
                    fiche.description = description;
                }
            }
            fiche.historique.push(narration.to_string());
        } else {
            etat.pnj_connus.push(FichePnj {
                nom: detecte.nom,
                description: detecte.nouvelle_description.unwrap_or_default(),
                historique: vec![narration.to_string()],
            });
        }
    }
}

pub struct JouerTour;

impl JouerTour {
    /// `de_joueur` lance uniquement les dés qui appartiennent au joueur
    /// (son jet d'attaque, son jet de dégâts) ; `de_monde` lance tout le
    /// reste (initiative des deux camps, jets des ennemis, et la
    /// rencontre initiale). Les séparer permet à l'appelant de brancher
    /// `de_joueur` sur un générateur qui attend une action du joueur (ex.
    /// un dé virtuel à l'écran dans `app_desktop`) sans jamais faire
    /// attendre le joueur pour un jet qui n'est pas le sien.
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de_joueur: &mut dyn GenerateurDe,
        de_monde: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        let mut resume = Vec::new();
        amorcer_rencontre_initiale(etat, de_monde, &mut resume);

        let contexte_tour = ContexteTour {
            etat: etat.clone(),
            saisie_joueur: saisie.to_string(),
        };
        let action_proposee = narrateur
            .proposer_action(&contexte_tour)
            .map_err(message_echec_narrateur)?;

        valider_action(&action_proposee, etat)?;

        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.tour_actuel += 1;
        }

        let fuite = matches!(action_proposee, ActionJoueur::Fuir);

        if fuite {
            resume.extend(resoudre_action_joueur(etat, &action_proposee, de_joueur));
        } else {
            let ennemi_avant = etat
                .combat_en_cours
                .as_ref()
                .map(|c| c.ennemi_agit_avant_joueur)
                .unwrap_or(false);

            if ennemi_avant {
                resume.extend(resoudre_action_ennemis(etat, de_monde));
                terminer_si_joueur_mort(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_joueur(etat, &action_proposee, de_joueur));
                    terminer_si_ennemis_morts(etat);
                }
            } else {
                resume.extend(resoudre_action_joueur(etat, &action_proposee, de_joueur));
                terminer_si_ennemis_morts(etat);
                if !combat_termine(etat) {
                    resume.extend(resoudre_action_ennemis(etat, de_monde));
                    terminer_si_joueur_mort(etat);
                }
            }
        }

        let contexte_resultat = ContexteResultat {
            etat: etat.clone(),
            action: action_proposee,
            resume_mecanique: resume.clone(),
        };
        let narration = narrateur
            .raconter_resultat(&contexte_resultat, sur_fragment)
            .map_err(message_echec_narrateur)?;

        etat.resumes.push(narration.clone());
        condenser_si_necessaire(etat, narrateur);
        enregistrer_pnj_si_pertinent(etat, &narration, narrateur);

        let vainqueur_combat = etat.combat_en_cours.as_ref().filter(|c| c.termine).and_then(|c| c.vainqueur);
        let partie_terminee = vainqueur_combat == Some(Vainqueur::Ennemis);
        if vainqueur_combat.is_some() && !partie_terminee {
            // Victoire ou fuite réussie : le combat est fini mais
            // l'aventure continue — on revient en exploration plutôt que
            // de laisser un `Combat` terminé bloquer la partie.
            etat.combat_en_cours = None;
        }

        Ok(ResultatTour {
            narration,
            resume_mecanique: resume,
            vainqueur_combat,
            partie_terminee,
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
            pnj_connus: vec![],
            rencontre_initiale_declenchee: true,
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
    fn attaquer_qui_rate_ne_lance_jamais_le_de_de_degats() {
        // Un seul résultat programmé (le jet d'attaque) : si le code
        // tentait quand même de lancer les dégâts après un raté, ce
        // deuxième `.lancer()` ferait paniquer `GenerateurDeSeed`
        // ("plus de résultats programmés") — avec un dé réel automatique
        // ça ne changeait rien (lancé puis ignoré), mais avec le dé du
        // joueur (voir `worker::GenerateurDeJoueur`), ça lui demanderait un
        // jet de dégâts pour une attaque déjà ratée.
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![5]); // d20=5 + mod_dex(14)=2 => 7 < défense 12 (manqué)
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de);
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

    use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};

    struct NarrateurDeTest {
        action: ActionJoueur,
    }

    impl Narrateur for NarrateurDeTest {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(self.action.clone())
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            sur_fragment("narration de test");
            Ok("narration de test".to_string())
        }

        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok("résumé condensé de test".to_string())
        }

        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            sur_fragment("introduction de test");
            Ok("introduction de test".to_string())
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
    fn jouer_tour_joueur_tue_l_ennemi_et_revient_en_exploration() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemis[0].pv_actuels = 1;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3 (mains nues, tue le gobelin à 1 pv)
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(resultat.vainqueur_combat, Some(Vainqueur::Joueur));
        assert!(!resultat.partie_terminee, "une victoire ne doit pas terminer la partie");
        assert!(etat.combat_en_cours.is_none(), "le combat gagné doit laisser la place à l'exploration");
    }

    #[test]
    fn jouer_tour_fuite_reussie_revient_en_exploration() {
        let mut etat = etat_en_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Fuir };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "je fuis", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(resultat.vainqueur_combat, Some(Vainqueur::Fuite));
        assert!(!resultat.partie_terminee, "une fuite réussie ne doit pas terminer la partie");
        assert!(etat.combat_en_cours.is_none(), "la fuite doit laisser la place à l'exploration");
    }

    #[test]
    fn jouer_tour_mort_du_joueur_termine_reellement_la_partie() {
        let mut etat = etat_en_combat();
        etat.personnage.subir_degats(etat.personnage.pv_max); // 0 PV
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        // ennemi_agit_avant_joueur est false (étape par défaut) : l'action
        // libre du joueur ne consomme pas de dé, puis l'ennemi attaque
        // (jet d'attaque + jet de dégâts) — déjà sans effet, le joueur
        // étant déjà à 0 PV, mais `resoudre_action_ennemis` consomme ces
        // deux dés avant même de vérifier la mort.
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 4]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(resultat.vainqueur_combat, Some(Vainqueur::Ennemis));
        assert!(resultat.partie_terminee, "la mort du joueur doit terminer la partie");
        assert!(
            etat.combat_en_cours.is_some(),
            "contrairement à une victoire/fuite, le combat perdu n'a pas besoin d'être nettoyé : la partie est finie"
        );
    }

    #[test]
    fn jouer_tour_ennemi_plus_rapide_agit_avant_le_joueur() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemi_agit_avant_joueur = true;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        // ennemi: d20=10 + bonus_attaque 3 = 13 >= defense joueur 12 (touché) ; dégâts d6=4
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 4]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert_eq!(resultat.vainqueur_combat, None);
    }

    #[test]
    fn jouer_tour_hors_combat_ne_signale_jamais_de_vainqueur() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(resultat.vainqueur_combat, None);
        assert!(!resultat.partie_terminee);
    }

    #[test]
    fn jouer_tour_rejette_une_action_invalide_sans_consommer_de_de() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'attaque", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {});
        assert!(matches!(resultat, Err(ErreurTour::AucunCombatEnCours)));
    }

    #[test]
    fn jouer_tour_declenche_la_rencontre_initiale_au_tout_premier_tour() {
        let mut etat = etat_sans_combat();
        etat.rencontre_initiale_declenchee = false;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        // de_monde : initiative joueur (10), initiative gobelin (5) pour
        // `amorcer_rencontre_initiale`, puis attaque (1) + dégâts (1) du
        // gobelin qui agit ensuite (le joueur, plus rapide, agit en
        // premier avec une action libre qui ne consomme aucun dé).
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 5, 1, 1]);
        let resultat =
            JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {})
                .unwrap();

        assert!(etat.combat_en_cours.is_some(), "la rencontre doit démarrer dès le premier tour");
        assert!(etat.rencontre_initiale_declenchee);
        assert!(resultat.resume_mecanique.iter().any(|l| l.contains("Gobelin")));
    }

    #[test]
    fn jouer_tour_ne_redeclenche_pas_la_rencontre_si_deja_amorcee() {
        let mut etat = etat_en_combat(); // rencontre_initiale_declenchee: true (fixture)
        let pv_gobelin_avant = etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        // Seulement le jet d'attaque + dégâts du gobelin, qui agit après
        // le joueur (action libre, ne consomme aucun dé). Si la rencontre
        // se redéclenchait, `commencer_combat` consommerait deux dés
        // d'initiative EN PLUS ici — il n'y en a aucun de programmé : un
        // déclenchement en trop ferait paniquer `GenerateurDeSeed`.
        let mut de_monde = GenerateurDeSeed::nouveau(vec![1, 1]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels, pv_gobelin_avant);
    }

    use std::cell::RefCell;

    struct NarrateurEspion {
        action: ActionJoueur,
        contexte_resultat_recu: RefCell<Option<ContexteResultat>>,
    }

    impl Narrateur for NarrateurEspion {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(self.action.clone())
        }

        fn raconter_resultat(
            &mut self,
            contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            *self.contexte_resultat_recu.borrow_mut() = Some(contexte.clone());
            Ok(String::new())
        }

        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }
    }

    struct NarrateurCondensateur {
        resultat: Result<String, ErreurNarrateur>,
        anciens_recus: RefCell<Option<Vec<String>>>,
    }

    impl Narrateur for NarrateurCondensateur {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok("Il ne se passe rien de notable.".to_string())
        }

        fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
            *self.anciens_recus.borrow_mut() = Some(anciens.to_vec());
            match &self.resultat {
                Ok(texte) => Ok(texte.clone()),
                Err(ErreurNarrateur::EchecGeneration(message)) => {
                    Err(ErreurNarrateur::EchecGeneration(message.clone()))
                }
            }
        }

        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            Ok(None)
        }

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }
    }

    struct NarrateurDetectionPnj {
        resultat: Result<Option<PnjDetecte>, ErreurNarrateur>,
    }

    impl Narrateur for NarrateurDetectionPnj {
        fn proposer_action(&mut self, _contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
            Ok(ActionJoueur::ActionLibre("j'observe".into()))
        }

        fn raconter_resultat(
            &mut self,
            _contexte: &ContexteResultat,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok("Aldric le forgeron te salue.".to_string())
        }

        fn condenser_resumes(&mut self, _anciens: &[String]) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }

        fn detecter_pnj(&mut self, _narration: &str, _noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
            match &self.resultat {
                Ok(valeur) => Ok(valeur.clone()),
                Err(ErreurNarrateur::EchecGeneration(message)) => {
                    Err(ErreurNarrateur::EchecGeneration(message.clone()))
                }
            }
        }

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            Ok(String::new())
        }
    }

    fn resume_de_n_mots(n: usize) -> String {
        "mot ".repeat(n).trim_end().to_string()
    }

    #[test]
    fn jouer_tour_ajoute_la_narration_aux_resumes() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(etat.resumes, vec!["narration de test".to_string()]);
    }

    #[test]
    fn jouer_tour_condense_les_anciens_resumes_une_fois_le_seuil_depasse() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..9).map(|_| resume_de_n_mots(90)).collect(); // 9 * 90 = 810 mots, déjà au-dessus du seuil
        let mut narrateur = NarrateurCondensateur {
            resultat: Ok("résumé condensé".to_string()),
            anciens_recus: RefCell::new(None),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        // 9 anciens + 1 nouvelle narration = 10 résumés avant condensation ;
        // les NB_RESUMES_RECENTS_CONSERVES=5 derniers sont gardés, les 5 plus anciens condensés.
        assert_eq!(etat.resumes.len(), 1 + NB_RESUMES_RECENTS_CONSERVES);
        assert_eq!(etat.resumes[0], "résumé condensé");

        let anciens_recus = narrateur.anciens_recus.borrow();
        assert_eq!(anciens_recus.as_ref().unwrap().len(), 5);
    }

    #[test]
    fn jouer_tour_laisse_les_resumes_inchanges_si_la_condensation_echoue() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..9).map(|_| resume_de_n_mots(90)).collect();
        let resumes_avant = etat.resumes.clone();
        let mut narrateur = NarrateurCondensateur {
            resultat: Err(ErreurNarrateur::EchecGeneration("échec simulé".into())),
            anciens_recus: RefCell::new(None),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {});

        assert!(resultat.is_ok());
        let mut resumes_attendus = resumes_avant;
        resumes_attendus.push("Il ne se passe rien de notable.".to_string());
        assert_eq!(etat.resumes, resumes_attendus);
    }

    #[test]
    fn jouer_tour_rejette_un_resultat_de_condensation_vide() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..9).map(|_| resume_de_n_mots(90)).collect();
        let resumes_avant = etat.resumes.clone();
        let mut narrateur = NarrateurCondensateur {
            resultat: Ok("   ".to_string()),
            anciens_recus: RefCell::new(None),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        let mut resumes_attendus = resumes_avant;
        resumes_attendus.push("Il ne se passe rien de notable.".to_string());
        assert_eq!(etat.resumes, resumes_attendus);
    }

    #[test]
    fn jouer_tour_tronque_sans_ia_si_la_condensation_echoue_au_dela_de_la_limite_dure() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..LIMITE_DURE_RESUMES).map(|_| resume_de_n_mots(90)).collect();
        let mut narrateur = NarrateurCondensateur {
            resultat: Err(ErreurNarrateur::EchecGeneration("échec simulé".into())),
            anciens_recus: RefCell::new(None),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.resumes.len(), NB_RESUMES_RECENTS_CONSERVES);
    }

    #[test]
    fn jouer_tour_ne_condense_pas_si_sous_le_seuil_de_mots_malgre_plus_de_cinq_resumes() {
        let mut etat = etat_sans_combat();
        etat.resumes = (0..NB_RESUMES_RECENTS_CONSERVES + 2).map(|_| resume_de_n_mots(10)).collect();
        let resumes_avant = etat.resumes.clone();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre("j'observe".into()) };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        let mut resumes_attendus = resumes_avant;
        resumes_attendus.push("narration de test".to_string());
        assert_eq!(etat.resumes, resumes_attendus);
    }

    #[test]
    fn jouer_tour_informe_la_narration_avec_le_resultat_mecanique_reel() {
        let mut etat = etat_en_combat();
        let mut narrateur = NarrateurEspion {
            action: ActionJoueur::Attaquer { cible: "Gobelin".into() },
            contexte_resultat_recu: RefCell::new(None),
        };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3 (mains nues)
        // le gobelin (7 pv) survit, donc il agit ensuite : d20=1 + bonus_attaque 3 = 4 < défense 12 (manqué)
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![1, 1]);
        JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        let contexte_recu = narrateur.contexte_resultat_recu.borrow();
        let contexte_recu = contexte_recu.as_ref().expect("raconter_resultat aurait dû être appelé");
        assert!(contexte_recu.resume_mecanique.iter().any(|l| l.contains("touché")));
        assert_eq!(contexte_recu.action, ActionJoueur::Attaquer { cible: "Gobelin".into() });
    }

    #[test]
    fn jouer_tour_cree_une_fiche_pnj_pour_un_nouveau_pnj_detecte() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru et méfiant".to_string()),
            })),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je parle au forgeron", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus.len(), 1);
        assert_eq!(etat.pnj_connus[0].nom, "Aldric");
        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
        assert_eq!(etat.pnj_connus[0].historique, vec!["Aldric le forgeron te salue.".to_string()]);
    }

    #[test]
    fn jouer_tour_ajoute_a_l_historique_d_une_fiche_pnj_existante() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru et méfiant".to_string(),
            historique: vec!["Première rencontre à la forge.".to_string()],
        });
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte { nom: "Aldric".to_string(), nouvelle_description: None })),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je retourne voir Aldric", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus.len(), 1);
        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
        assert_eq!(
            etat.pnj_connus[0].historique,
            vec!["Première rencontre à la forge.".to_string(), "Aldric le forgeron te salue.".to_string()]
        );
    }

    #[test]
    fn jouer_tour_complete_la_description_d_une_fiche_pnj_existante_si_elle_etait_vide() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: String::new(),
            historique: vec!["Première rencontre.".to_string()],
        });
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru et méfiant".to_string()),
            })),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je retourne voir Aldric", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus.len(), 1);
        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
    }

    #[test]
    fn jouer_tour_ne_remplace_pas_une_description_deja_presente() {
        let mut etat = etat_sans_combat();
        etat.pnj_connus.push(FichePnj {
            nom: "Aldric".to_string(),
            description: "forgeron bourru et méfiant".to_string(),
            historique: vec!["Première rencontre.".to_string()],
        });
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("autre description".to_string()),
            })),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "je retourne voir Aldric", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert_eq!(etat.pnj_connus[0].description, "forgeron bourru et méfiant");
    }

    #[test]
    fn jouer_tour_ne_detecte_pas_de_pnj_pendant_un_combat_actif() {
        let mut etat = etat_en_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Ok(Some(PnjDetecte {
                nom: "Aldric".to_string(),
                nouvelle_description: Some("forgeron bourru".to_string()),
            })),
        };
        // attaque: d20=10 + mod_dex(14)=2 => 12 >= defense 12 (touché) ; dégâts d4=3
        // le gobelin (7 pv) survit, il agit ensuite : d20=1 + bonus_attaque 3 = 4 < défense 12 (manqué)
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![1, 1]);
        JouerTour::executer(&mut etat, "j'attaque le gobelin", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();

        assert!(etat.pnj_connus.is_empty());
    }

    #[test]
    fn jouer_tour_laisse_pnj_connus_inchange_si_la_detection_echoue() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDetectionPnj {
            resultat: Err(ErreurNarrateur::EchecGeneration("échec simulé".into())),
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {});

        assert!(resultat.is_ok());
        assert!(etat.pnj_connus.is_empty());
    }
}
