use crate::ports::generateur_de::GenerateurDe;
use crate::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
use crate::usecases::commencer_combat::commencer_combat;
use domain::{
    evaluer_jet_attaque, gabarit_ennemi, modificateur, ActionJoueur, Acteur, De, Difficulte, EffetObjet, EtatPartie,
    FichePnj, JetDe, LigneTranscript, Vainqueur, BONUS_MAITRISE,
};

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
        ActionJoueur::Esquiver => {
            if etat.combat_en_cours.is_some() {
                Ok(())
            } else {
                Err(ErreurTour::AucunCombatEnCours)
            }
        }
        ActionJoueur::ActionLibre { .. } => Ok(()),
        ActionJoueur::ProvoquerCombat { .. } => Ok(()),
    }
}

/// Lance un dé avec désavantage (D&D : deux jets, on garde le total le plus
/// bas) — utilisé pour les attaques ennemies pendant qu'une esquive du
/// joueur est active, et pour une action libre *agressive* du joueur en
/// combat (une tentative improvisée n'a pas la précision d'une vraie
/// attaque) ; une action libre non-agressive en combat ne lance aucun dé.
fn lancer_desavantage(de: &mut dyn GenerateurDe, type_de: De, modificateur: i32) -> JetDe {
    let premier = de.lancer(type_de, modificateur);
    let second = de.lancer(type_de, modificateur);
    if premier.total <= second.total {
        premier
    } else {
        second
    }
}

/// Lance les dégâts d'une attaque : relancés une deuxième fois et sommés
/// (pas multipliés) en cas de critique, comme en D&D 5e — le modificateur
/// n'est appliqué qu'une fois.
fn lancer_degats(de: &mut dyn GenerateurDe, type_de: De, critique: bool) -> JetDe {
    if !critique {
        return de.lancer(type_de, 0);
    }
    let premier = de.lancer(type_de, 0);
    let second = de.lancer(type_de, 0);
    JetDe::nouveau(premier.resultat + second.resultat, 0)
}

pub fn resoudre_action_joueur(
    etat: &mut EtatPartie,
    action: &ActionJoueur,
    de_joueur: &mut dyn GenerateurDe,
    de_monde: &mut dyn GenerateurDe,
) -> Vec<String> {
    let de = de_joueur;
    let mut resume = Vec::new();
    match action {
        ActionJoueur::Attaquer { cible } => {
            let modificateur_attaque = modificateur(etat.personnage.caracteristiques.dexterite) + BONUS_MAITRISE;
            let jet_attaque = de.lancer(De { faces: 20 }, modificateur_attaque);
            let arme = etat.personnage.arme_equipee();

            if let Some(combat) = etat.combat_en_cours.as_mut() {
                if let Some(ennemi) = combat.trouver_ennemi_vivant_mut(cible) {
                    let evaluation = evaluer_jet_attaque(jet_attaque, ennemi.defense);
                    if evaluation.touche {
                        let jet_degats = lancer_degats(de, arme, evaluation.critique);
                        let degats = jet_degats.total.max(0);
                        ennemi.subir_degats(degats);
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — touché{} ! Dégâts : {}",
                            ennemi.nom,
                            jet_attaque.total,
                            ennemi.defense,
                            if evaluation.critique { " (critique)" } else { "" },
                            degats
                        ));
                    } else {
                        resume.push(format!(
                            "🎲 Tu attaques {} : {} vs défense {} — {}",
                            ennemi.nom,
                            jet_attaque.total,
                            ennemi.defense,
                            if jet_attaque.resultat == 1 { "échec critique" } else { "manqué" }
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
        ActionJoueur::Esquiver => {
            if let Some(combat) = etat.combat_en_cours.as_mut() {
                combat.joueur_esquive = true;
            }
            resume.push("🛡️ Tu te mets en garde, prêt à esquiver la prochaine attaque.".to_string());
        }
        ActionJoueur::ActionLibre { texte, agressif } => {
            // Avant, une action libre en combat n'avait AUCUN effet
            // mécanique (ni jet, ni dégâts), même quand le récit généré
            // décrivait quelque chose — c'est la cause du rapport « mes
            // actions ne sont pas prises en compte ». Puis, toute action
            // créative pendant un combat actif devenait une attaque
            // improvisée réelle, même une action manifestement pacifique
            // (négocier, observer...) — c'est maintenant corrigé : seule
            // une action libre jugée agressive par le classificateur
            // devient une attaque improvisée contre le premier ennemi
            // vivant, avec désavantage (elle n'a pas la précision d'une
            // vraie attaque) ; une action libre non-agressive en combat
            // reste purement narrative, sans aucun jet de dé. Hors combat,
            // une action libre reste toujours purement narrative, quelle
            // que soit son agressivité.
            let combat_actif = etat.combat_en_cours.as_ref().map(|c| !c.termine).unwrap_or(false);
            if combat_actif && *agressif {
                let modificateur_attaque = modificateur(etat.personnage.caracteristiques.dexterite) + BONUS_MAITRISE;
                let arme = etat.personnage.arme_equipee();
                if let Some(combat) = etat.combat_en_cours.as_mut() {
                    if let Some(ennemi) = combat.ennemis.iter_mut().find(|e| !e.est_mort()) {
                        let jet_attaque = lancer_desavantage(de, De { faces: 20 }, modificateur_attaque);
                        let evaluation = evaluer_jet_attaque(jet_attaque, ennemi.defense);
                        if evaluation.touche {
                            let jet_degats = lancer_degats(de, arme, evaluation.critique);
                            let degats = jet_degats.total.max(0);
                            ennemi.subir_degats(degats);
                            resume.push(format!(
                                "🎲 Tentative improvisée (« {} ») contre {} : {} vs défense {} — touché{} ! Dégâts : {}",
                                texte,
                                ennemi.nom,
                                jet_attaque.total,
                                ennemi.defense,
                                if evaluation.critique { " (critique)" } else { "" },
                                degats
                            ));
                        } else {
                            resume.push(format!(
                                "🎲 Tentative improvisée (« {} ») contre {} : {} vs défense {} — {}",
                                texte,
                                ennemi.nom,
                                jet_attaque.total,
                                ennemi.defense,
                                if jet_attaque.resultat == 1 { "échec critique" } else { "manqué" }
                            ));
                        }
                    }
                }
            } else if combat_actif && !*agressif {
                resume.push(format!("💬 Tentative non-agressive : « {texte} »."));
            }
        }
        ActionJoueur::ProvoquerCombat { ennemi, difficulte } => {
            if etat.combat_en_cours.is_none() {
                let nouvel_ennemi = gabarit_ennemi(ennemi.clone(), *difficulte);
                let combat = commencer_combat(&etat.personnage, vec![nouvel_ennemi], de_monde);
                etat.combat_en_cours = Some(combat);
                resume.push(format!("⚔️ {ennemi} surgit et t'attaque !"));
            } else {
                // Un combat est déjà actif : `ennemi` est censé nommer un
                // adversaire déjà engagé, mais un petit modèle local peut se
                // tromper de nom (ex. "Loup" alors que le combat oppose un
                // "Gobelin"). `valider_action` accepte toujours
                // "provoquer_combat" sans vérifier `ennemi`, donc le garde-fou
                // `CibleInvalide` qui protège une vraie `Attaquer` ne joue
                // jamais ici — sans ce rattrapage, `Attaquer` ci-dessous ne
                // trouverait aucune cible via `trouver_ennemi_vivant_mut` et
                // ne pousserait rien au résumé : un tour silencieusement
                // perdu. On retombe donc sur le premier ennemi vivant du
                // combat en cours quand le nom proposé ne correspond à
                // personne (même repli que pour l'attaque improvisée d'une
                // action libre agressive, plus haut dans cette fonction).
                let cible_nommee_existe = etat
                    .combat_en_cours
                    .as_ref()
                    .map(|c| c.ennemis.iter().any(|e| &e.nom == ennemi && !e.est_mort()))
                    .unwrap_or(false);
                let cible = if cible_nommee_existe {
                    Some(ennemi.clone())
                } else {
                    etat.combat_en_cours
                        .as_ref()
                        .and_then(|c| c.ennemis.iter().find(|e| !e.est_mort()))
                        .map(|e| e.nom.clone())
                };
                if let Some(cible) = cible {
                    resume.extend(resoudre_action_joueur(
                        etat,
                        &ActionJoueur::Attaquer { cible },
                        de,
                        de_monde,
                    ));
                }
            }
        }
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

/// Résout l'attaque d'un seul ennemi (par son indice dans `combat.ennemis`)
/// contre le joueur. Remplace l'ancienne `resoudre_action_ennemis`, qui
/// résolvait tous les ennemis d'un bloc avant ou après le joueur — voir
/// `Combat::ordre_de_tour`, qui classe désormais chaque combattant
/// individuellement et appelle cette fonction une fois par ennemi, dans
/// l'ordre.
pub fn resoudre_attaque_ennemi(etat: &mut EtatPartie, de: &mut dyn GenerateurDe, indice: usize) -> Vec<String> {
    let mut resume = Vec::new();
    let defense_joueur = etat.personnage.defense();
    let desavantage = etat.combat_en_cours.as_ref().map(|c| c.joueur_esquive).unwrap_or(false);

    let Some(combat) = etat.combat_en_cours.as_ref() else {
        return resume;
    };
    if combat.termine {
        return resume;
    }
    let Some(ennemi) = combat.ennemis.get(indice) else {
        return resume;
    };
    if ennemi.est_mort() {
        return resume;
    }
    // Copie des champs nécessaires avant de lâcher l'emprunt immuable sur
    // `combat` : `etat.personnage.subir_degats` emprunte `etat` plus bas.
    let (nom, bonus_attaque, degats_de) = (ennemi.nom.clone(), ennemi.bonus_attaque, ennemi.degats);

    let jet_attaque = if desavantage {
        lancer_desavantage(de, De { faces: 20 }, bonus_attaque)
    } else {
        de.lancer(De { faces: 20 }, bonus_attaque)
    };
    let evaluation = evaluer_jet_attaque(jet_attaque, defense_joueur);

    if evaluation.touche {
        let jet_degats = lancer_degats(de, degats_de, evaluation.critique);
        let degats = jet_degats.total.max(0);
        etat.personnage.subir_degats(degats);
        resume.push(format!(
            "🎲 {} t'attaque : {} vs défense {} — touché{} ! Dégâts : {}",
            nom,
            jet_attaque.total,
            defense_joueur,
            if evaluation.critique { " (critique)" } else { "" },
            degats
        ));
    } else {
        resume.push(format!(
            "🎲 {} t'attaque : {} vs défense {} — {}",
            nom,
            jet_attaque.total,
            defense_joueur,
            if jet_attaque.resultat == 1 { "échec critique" } else { "manqué" }
        ));
    }
    resume
}

fn combat_termine(etat: &EtatPartie) -> bool {
    etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(true)
}

/// `true` seulement si un combat est actif ET déjà conclu — à distinguer
/// de `combat_termine`, qui renvoie aussi `true` quand il n'y a pas de
/// combat du tout (pratique pour les appelants qui veulent dire « plus
/// rien à résoudre côté combat », mais inutilisable comme condition
/// d'arrêt de la boucle de résolution d'un tour : hors combat, elle
/// arrêterait tout avant même l'action du joueur).
fn combat_actif_et_conclu(etat: &EtatPartie) -> bool {
    etat.combat_en_cours.as_ref().map(|c| c.termine).unwrap_or(false)
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
    /// reste (initiative des deux camps et jets des ennemis). Les séparer
    /// permet à l'appelant de brancher `de_joueur` sur un générateur qui
    /// attend une action du joueur (ex. un dé virtuel à l'écran dans
    /// `app_desktop`) sans jamais faire attendre le joueur pour un jet qui
    /// n'est pas le sien.
    pub fn executer(
        etat: &mut EtatPartie,
        saisie: &str,
        narrateur: &mut dyn Narrateur,
        de_joueur: &mut dyn GenerateurDe,
        de_monde: &mut dyn GenerateurDe,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<ResultatTour, ErreurTour> {
        // Enregistrée avant toute validation, pour que la ligne du joueur
        // reste dans la transcript même si le tour échoue ensuite (action
        // invalide, erreur du narrateur) — elle a bien été dite, qu'elle
        // ait eu un effet mécanique ou non.
        etat.transcript.push(LigneTranscript::Joueur(saisie.to_string()));

        let mut resume = Vec::new();

        let contexte_tour = ContexteTour {
            etat: etat.clone(),
            saisie_joueur: saisie.to_string(),
        };
        let action_proposee = narrateur
            .proposer_action(&contexte_tour)
            .map_err(message_echec_narrateur)?;

        // Filet de sécurité mécanique : le prompt de classification
        // (`construire_prompt_classification` dans `infra`) ne propose
        // désormais "attaquer" que lorsqu'un combat est déjà actif, mais un
        // petit modèle local (ex. 3B quantifié) ne suit pas toujours le
        // schéma à la lettre. Si le narrateur répond quand même "attaquer"
        // alors qu'aucun combat n'est en cours, on retombe sur
        // "provoquer_combat" plutôt que de laisser `valider_action` rejeter
        // ce qui était, de l'intention du joueur, un déclenchement de
        // combat — l'intention la plus courante derrière "j'attaque X" en
        // dehors d'un combat.
        let action_proposee = match action_proposee {
            ActionJoueur::Attaquer { cible } if etat.combat_en_cours.is_none() => {
                ActionJoueur::ProvoquerCombat { ennemi: cible, difficulte: Difficulte::Faible }
            }
            autre => autre,
        };

        valider_action(&action_proposee, etat)?;

        if let Some(combat) = etat.combat_en_cours.as_mut() {
            combat.tour_actuel += 1;
        }

        let fuite = matches!(action_proposee, ActionJoueur::Fuir);

        if fuite {
            resume.extend(resoudre_action_joueur(etat, &action_proposee, de_joueur, de_monde));
        } else {
            // Ordre d'initiative complet (joueur + chaque ennemi
            // individuellement, voir `Combat::ordre_de_tour`) plutôt que
            // l'ancien booléen qui groupait tous les ennemis avant ou
            // après le joueur. Hors combat, un seul acteur : le joueur.
            let ordre = etat
                .combat_en_cours
                .as_ref()
                .map(|c| c.ordre_de_tour())
                .unwrap_or_else(|| vec![Acteur::Joueur]);

            for acteur in ordre {
                if combat_actif_et_conclu(etat) {
                    break;
                }
                match acteur {
                    Acteur::Joueur => {
                        // La protection d'une esquive déclarée au tour
                        // précédent s'arrête au début de ce nouveau tour
                        // du joueur ("jusqu'au début de ton tour suivant"
                        // en D&D) — elle a déjà protégé les éventuelles
                        // attaques ennemies rencontrées depuis.
                        if let Some(combat) = etat.combat_en_cours.as_mut() {
                            combat.joueur_esquive = false;
                        }
                        resume.extend(resoudre_action_joueur(etat, &action_proposee, de_joueur, de_monde));
                        terminer_si_ennemis_morts(etat);
                    }
                    Acteur::Ennemi(indice) => {
                        resume.extend(resoudre_attaque_ennemi(etat, de_monde, indice));
                        terminer_si_joueur_mort(etat);
                    }
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
        etat.transcript.push(LigneTranscript::Narrateur(narration.clone()));
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
            premisse: Default::default(),
            transcript: vec![],
        }
    }

    fn etat_en_combat() -> EtatPartie {
        let mut etat = etat_sans_combat();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0], // joueur plus rapide => il agit en premier (ancien défaut `ennemi_agit_avant_joueur: false`)
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
            joueur_esquive: false,
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
    fn esquiver_sans_combat_est_invalide() {
        let resultat = valider_action(&ActionJoueur::Esquiver, &etat_sans_combat());
        assert_eq!(resultat, Err(ErreurTour::AucunCombatEnCours));
    }

    #[test]
    fn esquiver_en_combat_est_valide() {
        let resultat = valider_action(&ActionJoueur::Esquiver, &etat_en_combat());
        assert_eq!(resultat, Ok(()));
    }

    #[test]
    fn action_libre_est_toujours_valide() {
        let resultat = valider_action(
            &ActionJoueur::ActionLibre { texte: "Je regarde autour de moi".into(), agressif: false },
            &etat_sans_combat(),
        );
        assert_eq!(resultat, Ok(()));
    }

    use crate::ports::generateur_de::GenerateurDeSeed;
    use domain::Vainqueur;

    #[test]
    fn attaquer_qui_touche_infflige_des_degats() {
        let mut etat = etat_en_combat();
        // attaque: d20=10 + mod_dex(14)=2 + maîtrise 2 => 14 >= defense 12 (touché, pas critique)
        // dégâts: mains nues d4=3 (aucune arme en inventaire)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de, &mut de_monde);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 4); // 7 - 3
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn attaquer_qui_rate_n_infflige_aucun_degat() {
        let mut etat = etat_en_combat();
        // attaque: d20=5 + mod_dex(14)=2 + maîtrise 2 => 9 < defense 12 (manqué)
        let mut de = GenerateurDeSeed::nouveau(vec![5, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de, &mut de_monde);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 7);
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn attaquer_qui_rate_ne_lance_jamais_le_de_de_degats() {
        // Un seul résultat programmé (le jet d'attaque) : si le code
        // tentait quand même de lancer les dégâts après un raté, ce
        // deuxième `.lancer()` ferait paniquer `GenerateurDeSeed`
        // ("plus de résultats programmés").
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![5]); // d20=5 + 2 + 2 => 9 < défense 12 (manqué)
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de, &mut de_monde);
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn attaquer_avec_un_20_naturel_touche_toujours_et_double_les_des_de_degats() {
        // d20=20 touche toujours et déclenche un critique, même ici où le
        // total ne serait pas suffisant sans le bonus : dégâts = deux d4
        // relancés et sommés (mains nues), pas doublés arithmétiquement.
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![20, 2, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de, &mut de_monde);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 2); // 7 - (2 + 3)
        assert!(resume[0].contains("critique"));
    }

    #[test]
    fn attaquer_avec_un_1_naturel_rate_toujours_meme_si_le_total_aurait_suffi() {
        // mod_dex(14)=2 + maîtrise 2 = 4, donc un 1 donnerait un total de 5
        // — largement insuffisant ici de toute façon, mais le point est
        // que le résultat brut de 1 est ce qui décide, pas le total.
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![1]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Attaquer { cible: "Gobelin".into() }, &mut de, &mut de_monde);
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 7);
        assert!(resume[0].contains("échec critique"));
    }

    #[test]
    fn esquiver_active_le_flag_sur_le_combat() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::Esquiver, &mut de, &mut de_monde);
        assert!(etat.combat_en_cours.as_ref().unwrap().joueur_esquive);
        assert!(resume[0].contains("esquiver"));
    }

    #[test]
    fn action_libre_hors_combat_ne_consomme_aucun_de() {
        let mut etat = etat_sans_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ActionLibre { texte: "J'observe".into(), agressif: false },
            &mut de,
            &mut de_monde,
        );
        assert!(resume.is_empty());
    }

    #[test]
    fn action_libre_en_combat_devient_une_attaque_improvisee_qui_peut_infliger_des_degats() {
        // Avant cette correction, une action libre en combat n'avait
        // aucun effet mécanique — c'était le cœur du rapport « mes
        // actions ne comptent pas ». Ici, l'action est agressive ("je lui
        // jette du sable"), donc elle devient une attaque improvisée.
        // Désavantage : deux jets, on garde le plus bas (10 < 15) => 10 +
        // mod_dex(2) + maîtrise(2) = 14 >= 12 (touché) ; dégâts d4=3.
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![10, 15, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ActionLibre { texte: "je lui jette du sable".into(), agressif: true },
            &mut de,
            &mut de_monde,
        );
        let gobelin = &etat.combat_en_cours.as_ref().unwrap().ennemis[0];
        assert_eq!(gobelin.pv_actuels, 4); // 7 - 3
        assert!(resume[0].contains("touché"));
        assert!(resume[0].contains("je lui jette du sable"));
    }

    #[test]
    fn action_libre_en_combat_rate_ne_lance_jamais_le_de_de_degats() {
        // `agressif: true` ici pour exercer la branche d'attaque improvisée
        // (le texte "je recule" sert juste de contenu de test, peu importe
        // son agressivité réelle). Désavantage : deux 5 => 5 + 2 + 2 = 9 <
        // défense 12 (manqué). Un seul couple de valeurs programmé pour le
        // jet d'attaque ; un troisième `.lancer()` pour des dégâts ferait
        // paniquer `GenerateurDeSeed`.
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![5, 5]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ActionLibre { texte: "je recule".into(), agressif: true },
            &mut de,
            &mut de_monde,
        );
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn action_libre_agressive_en_combat_devient_une_attaque_improvisee() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![15, 15, 4]); // désavantage garde 15, dégâts 4
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ActionLibre { texte: "je frappe avec une chaise".into(), agressif: true },
            &mut de,
            &mut de_monde,
        );
        assert!(resume.iter().any(|l| l.contains("Tentative improvisée")));
        assert!(etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels < 7);
    }

    #[test]
    fn action_libre_non_agressive_en_combat_ne_lance_aucun_de() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]); // doit paniquer si un dé est lancé
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ActionLibre { texte: "je négocie avec le gobelin".into(), agressif: false },
            &mut de,
            &mut de_monde,
        );
        assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis[0].pv_actuels, 7);
        assert!(resume.iter().any(|l| l.contains("non-agressive")));
    }

    #[test]
    fn utiliser_objet_soigne_et_retire_l_objet_de_l_inventaire() {
        let mut etat = etat_sans_combat();
        etat.personnage.subir_degats(5);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(&mut etat, &ActionJoueur::UtiliserObjet { objet: "Potion de soin".into() }, &mut de, &mut de_monde);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max);
        assert!(etat.personnage.inventaire.is_empty());
        assert!(resume[0].contains("Potion de soin"));
    }

    #[test]
    fn fuir_termine_le_combat_avec_le_vainqueur_fuite() {
        let mut etat = etat_en_combat();
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        resoudre_action_joueur(&mut etat, &ActionJoueur::Fuir, &mut de, &mut de_monde);
        let combat = etat.combat_en_cours.as_ref().unwrap();
        assert!(combat.termine);
        assert_eq!(combat.vainqueur, Some(Vainqueur::Fuite));
    }

    #[test]
    fn provoquer_combat_sans_combat_en_cours_en_demarre_un_avec_le_bon_gabarit() {
        let mut etat = etat_sans_combat();
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 5]); // init joueur, init ennemi
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ProvoquerCombat { ennemi: "Loup".into(), difficulte: domain::Difficulte::Faible },
            &mut de_joueur,
            &mut de_monde,
        );
        let combat = etat.combat_en_cours.as_ref().expect("un combat doit démarrer");
        assert_eq!(combat.ennemis[0].nom, "Loup");
        assert_eq!(combat.ennemis[0].pv_max, 4); // palier Faible
        assert!(resume.iter().any(|l| l.contains("Loup")));
    }

    #[test]
    fn provoquer_combat_alors_qu_un_combat_est_deja_en_cours_attaque_au_lieu_de_planter() {
        let mut etat = etat_en_combat(); // contient déjà un Gobelin
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![15, 4]); // attaque, dégâts
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ProvoquerCombat { ennemi: "Gobelin".into(), difficulte: domain::Difficulte::Fort },
            &mut de_joueur,
            &mut de_monde,
        );
        assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis.len(), 1); // pas de doublon
        assert!(!resume.is_empty());
    }

    #[test]
    fn provoquer_combat_avec_un_nom_d_ennemi_errone_attaque_l_ennemi_reel_au_lieu_de_ne_rien_faire() {
        // Le combat en cours oppose un "Gobelin", mais le modèle propose
        // "provoquer_combat" avec `ennemi: "Loup"` (nom qui ne correspond à
        // aucun combattant réel). Avant ce correctif, le code recourait
        // toujours à `Attaquer { cible: ennemi.clone() }`, qui échoue à
        // trouver "Loup" via `trouver_ennemi_vivant_mut`, lance tout de même
        // le dé d'attaque, puis ne pousse RIEN au résumé — un tour
        // silencieusement perdu. Le rattrapage doit retomber sur le premier
        // ennemi vivant (le Gobelin) et produire un résumé non vide.
        let mut etat = etat_en_combat(); // contient un "Gobelin", pas de "Loup"
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![15, 4]); // attaque, dégâts
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_action_joueur(
            &mut etat,
            &ActionJoueur::ProvoquerCombat { ennemi: "Loup".into(), difficulte: domain::Difficulte::Faible },
            &mut de_joueur,
            &mut de_monde,
        );
        assert!(!resume.is_empty(), "le tour ne doit pas être silencieusement perdu");
        assert!(resume.iter().any(|l| l.contains("Gobelin")), "l'attaque doit viser l'ennemi réel (Gobelin)");
        assert_eq!(etat.combat_en_cours.as_ref().unwrap().ennemis.len(), 1); // pas de "Loup" ajouté
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
            _premisse: &domain::PremisseHistoire,
            sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, ErreurNarrateur> {
            sur_fragment("introduction de test");
            Ok("introduction de test".to_string())
        }
    }

    #[test]
    fn resoudre_attaque_ennemi_infflige_des_degats_au_joueur() {
        let mut etat = etat_en_combat();
        // ennemi (Gobelin, bonus_attaque 3) : d20=10 => total 13 >= défense joueur 12 (touché)
        let mut de = GenerateurDeSeed::nouveau(vec![10, 4]); // attaque puis dégâts (d6=4)
        let resume = resoudre_attaque_ennemi(&mut etat, &mut de, 0);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert!(resume[0].contains("touché"));
    }

    #[test]
    fn resoudre_attaque_ennemi_qui_rate_ne_lance_jamais_le_de_de_degats() {
        let mut etat = etat_en_combat();
        // d20=1 => résultat brut 1, rate toujours quel que soit le bonus.
        // Un seul résultat programmé : un deuxième `.lancer()` pour des
        // dégâts ferait paniquer `GenerateurDeSeed`.
        let mut de = GenerateurDeSeed::nouveau(vec![1]);
        let resume = resoudre_attaque_ennemi(&mut etat, &mut de, 0);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max);
        assert!(resume[0].contains("échec critique"));
    }

    #[test]
    fn resoudre_attaque_ennemi_sous_desavantage_garde_le_jet_le_plus_bas() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().joueur_esquive = true;
        // Désavantage : 15 puis 3 => on garde 3 (le plus bas) => 3+3=6 < 12
        // (manqué), malgré le 15 qui aurait largement touché seul.
        let mut de = GenerateurDeSeed::nouveau(vec![15, 3]);
        let resume = resoudre_attaque_ennemi(&mut etat, &mut de, 0);
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max);
        assert!(resume[0].contains("manqué"));
    }

    #[test]
    fn resoudre_attaque_ennemi_ne_fait_rien_si_combat_termine() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().terminer(Vainqueur::Joueur);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_attaque_ennemi(&mut etat, &mut de, 0);
        assert!(resume.is_empty());
    }

    #[test]
    fn resoudre_attaque_ennemi_ne_fait_rien_pour_un_ennemi_deja_mort() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemis[0].subir_degats(100);
        let mut de = GenerateurDeSeed::nouveau(vec![]);
        let resume = resoudre_attaque_ennemi(&mut etat, &mut de, 0);
        assert!(resume.is_empty());
    }

    #[test]
    fn jouer_tour_joueur_tue_l_ennemi_et_revient_en_exploration() {
        let mut etat = etat_en_combat();
        etat.combat_en_cours.as_mut().unwrap().ennemis[0].pv_actuels = 1;
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Gobelin".into() } };
        // attaque: d20=10 + mod_dex(2) + maîtrise(2) => 14 >= defense 12 (touché) ; dégâts d4=3 (mains nues, tue le gobelin à 1 pv)
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
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        // Le combat n'est pas encore flaggé `termine` au début de ce tour
        // (seul `terminer_si_joueur_mort`, appelé après le tour du
        // gobelin, le fera) : le tour se résout donc intégralement malgré
        // les 0 PV déjà présents. Action libre non-agressive en combat =>
        // aucun jet côté joueur ; puis le gobelin attaque et termine la
        // partie.
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
        etat.combat_en_cours.as_mut().unwrap().initiatives_ennemis = vec![20]; // bien plus rapide que l'initiative du joueur (10)
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        // ennemi: d20=10 + bonus_attaque 3 = 13 >= defense joueur 12 (touché) ; dégâts d6=4
        // puis le joueur agit : action libre non-agressive en combat =>
        // aucun jet côté joueur.
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 4]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 4);
        assert_eq!(resultat.vainqueur_combat, None);
    }

    #[test]
    fn jouer_tour_plusieurs_ennemis_agissent_chacun_a_leur_place_dans_l_ordre_d_initiative() {
        let mut etat = etat_en_combat();
        {
            let combat = etat.combat_en_cours.as_mut().unwrap();
            // Gobelin B (index 1), plus rapide (18) que le joueur (10),
            // agit avant lui ; Gobelin A (index 0, 0) agit après.
            combat.ennemis.push(Ennemi::nouveau("Gobelin B".into(), 7, De { faces: 6 }, 12, 2, 3));
            combat.initiatives_ennemis = vec![0, 18];
        }
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        // ordre attendu : Ennemi(1) [Gobelin B], Joueur, Ennemi(0) [Gobelin A]
        // Gobelin B : d20=10+3=13>=12 touché, dégâts d6=2
        // Joueur (action libre non-agressive) : aucun jet
        // Gobelin A : d20=1 => échec critique automatique, pas de dégâts
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 2, 1]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(etat.personnage.pv_actuels, etat.personnage.pv_max - 2);
        assert!(resultat.resume_mecanique.iter().any(|l| l.contains("Gobelin B") && l.contains("touché")));
        assert!(resultat.resume_mecanique.iter().any(|l| l.contains("échec critique")));
    }

    #[test]
    fn jouer_tour_hors_combat_ne_signale_jamais_de_vainqueur() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(resultat.vainqueur_combat, None);
        assert!(!resultat.partie_terminee);
    }

    #[test]
    fn jouer_tour_rejette_une_action_invalide_sans_consommer_de_de() {
        // `Attaquer` hors combat n'est plus un cas rejeté ici : voir
        // `jouer_tour_attaquer_hors_combat_demarre_un_combat_au_lieu_d_echouer`
        // ci-dessous, qui couvre précisément ce rattrapage. `UtiliserObjet`
        // avec un objet absent de l'inventaire reste, lui, toujours invalide.
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::UtiliserObjet { objet: "Objet inexistant".into() } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat = JouerTour::executer(&mut etat, "j'utilise un objet", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {});
        assert!(matches!(resultat, Err(ErreurTour::ObjetInvalide)));
    }

    #[test]
    fn jouer_tour_attaquer_hors_combat_demarre_un_combat_au_lieu_d_echouer() {
        // Régression centrale visée par ce correctif : avant, un joueur
        // disant « j'attaque le loup » hors combat pouvait être classifié
        // en `Attaquer { cible: "Loup" }` par le narrateur (le prompt
        // proposait ce format même hors combat) ; `valider_action` le
        // rejetait alors systématiquement avec `AucunCombatEnCours`, ce que
        // l'UI affichait comme une erreur brute au joueur — son intention
        // de déclencher un combat échouait silencieusement. `JouerTour::executer`
        // retombe désormais sur `ProvoquerCombat` quand `Attaquer` arrive
        // sans combat en cours : le tour doit réussir et démarrer un combat
        // contre "Loup", pas échouer.
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Loup".into() } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![10, 5]); // initiative joueur, initiative ennemi
        let resultat = JouerTour::executer(&mut etat, "j'attaque le loup", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {})
            .expect("le tour doit réussir : Attaquer hors combat doit démarrer un combat, pas échouer");
        assert_eq!(resultat.vainqueur_combat, None);
        let combat = etat.combat_en_cours.as_ref().expect("un combat doit avoir démarré contre le Loup");
        assert_eq!(combat.ennemis[0].nom, "Loup");
    }

    #[test]
    fn un_premier_tour_non_agressif_ne_demarre_aucun_combat() {
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest {
            action: ActionJoueur::ActionLibre { texte: "j'observe les environs".into(), agressif: false },
        };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe les environs", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {})
            .unwrap();
        assert!(etat.combat_en_cours.is_none());
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
            _premisse: &domain::PremisseHistoire,
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
            Ok(ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false })
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
            _premisse: &domain::PremisseHistoire,
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
            Ok(ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false })
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
            _premisse: &domain::PremisseHistoire,
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
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(etat.resumes, vec!["narration de test".to_string()]);
    }

    #[test]
    fn jouer_tour_ajoute_la_saisie_puis_la_narration_a_la_transcript() {
        // Review Focus : reprendre une histoire sauvegardée doit pouvoir
        // réafficher la conversation complète (les deux côtés), pas
        // seulement ce que `resumes` garde pour la mémoire du modèle.
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        JouerTour::executer(&mut etat, "j'observe", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {}).unwrap();
        assert_eq!(
            etat.transcript,
            vec![
                LigneTranscript::Joueur("j'observe".to_string()),
                LigneTranscript::Narrateur("narration de test".to_string()),
            ]
        );
    }

    #[test]
    fn jouer_tour_garde_la_saisie_dans_la_transcript_meme_si_le_tour_echoue() {
        // La saisie du joueur a bien été dite, qu'elle ait eu un effet
        // mécanique ou non — voir `jouer_tour_rejette_une_action_invalide_sans_consommer_de_de`
        // pour le cas d'échec exercé ici (cible inexistante hors combat).
        let mut etat = etat_sans_combat();
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::Attaquer { cible: "Fantôme".into() } };
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![]);
        let resultat =
            JouerTour::executer(&mut etat, "j'attaque le fantôme", &mut narrateur, &mut de_joueur, &mut de_monde, &mut |_| {});
        assert!(resultat.is_err());
        assert_eq!(etat.transcript, vec![LigneTranscript::Joueur("j'attaque le fantôme".to_string())]);
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
        let mut narrateur = NarrateurDeTest { action: ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false } };
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
        // attaque: d20=10 + mod_dex(2) + maîtrise(2) => 14 >= defense 12 (touché) ; dégâts d4=3 (mains nues)
        // le gobelin (7 pv) survit, donc il agit ensuite : d20=1 => échec critique automatique, pas de dégâts
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![1]);
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
        // attaque: d20=10 + mod_dex(2) + maîtrise(2) => 14 >= defense 12 (touché) ; dégâts d4=3
        // le gobelin (7 pv) survit, il agit ensuite : d20=1 => échec critique automatique
        let mut de_joueur = GenerateurDeSeed::nouveau(vec![10, 3]);
        let mut de_monde = GenerateurDeSeed::nouveau(vec![1]);
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
