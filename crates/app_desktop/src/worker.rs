use application::ports::generateur_de::GenerateurDe;
use application::ports::narrateur::Narrateur;
use application::ports::sauvegarde::{ErreurSauvegarde, Sauvegarde};
use application::usecases::creer_personnage::{
    creer_personnage, tirer_valeurs_caracteristiques, ErreurCreationPersonnage,
};
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, EtatPartie, Genre, JetDe};
use std::path::{Path, PathBuf};

use crate::histoires::{self, InfoHistoire};
use crate::protocole::{MessageVersUi, MessageVersWorker};

/// Répertoire de stockage privé de l'app sur Android — inchangé depuis le
/// portage Android (JNI `Activity.getFilesDir()`), voir le commentaire
/// historique si besoin de le retoucher. Partagé entre les deux fonctions
/// de répertoire ci-dessous.
#[cfg(feature = "plateforme-android")]
fn repertoire_donnees_app() -> PathBuf {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .expect("JavaVM Android valide fournie par le contexte natif");
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
    let mut env = vm
        .attach_current_thread()
        .expect("attachement du thread courant à la JVM Android");
    let fichier = env
        .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])
        .expect("appel de Activity.getFilesDir()")
        .l()
        .expect("getFilesDir() retourne un java.io.File");
    let chemin_jstring = env
        .call_method(&fichier, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .expect("appel de File.getAbsolutePath()")
        .l()
        .expect("getAbsolutePath() retourne une java.lang.String");
    // `JString::from(chemin_jstring)` crée une valeur temporaire ; la lier
    // avant d'appeler `get_string` (plutôt que de l'emboîter dans l'appel)
    // évite que le compilateur la libère avant que `chemin_java_str`
    // (qui l'emprunte via `JavaStr`) ne soit utilisée.
    let chemin_jstring = jni::objects::JString::from(chemin_jstring);
    let chemin_java_str = env
        .get_string(&chemin_jstring)
        .expect("conversion JString -> JavaStr");
    PathBuf::from(chemin_java_str.to_str().expect("UTF-8 valide"))
}

#[cfg(feature = "plateforme-desktop")]
pub fn repertoire_sauvegardes_par_defaut() -> PathBuf {
    PathBuf::from("sauvegardes")
}

#[cfg(feature = "plateforme-android")]
pub fn repertoire_sauvegardes_par_defaut() -> PathBuf {
    repertoire_donnees_app().join("sauvegardes")
}

#[cfg(feature = "plateforme-desktop")]
pub fn repertoire_modeles_par_defaut() -> PathBuf {
    PathBuf::from("modeles")
}

#[cfg(feature = "plateforme-android")]
pub fn repertoire_modeles_par_defaut() -> PathBuf {
    repertoire_donnees_app().join("modeles")
}

/// Construit l'état initial d'une nouvelle histoire (personnage, sans
/// combat), sans rien écrire sur disque ni toucher à l'index — pure,
/// testable isolément. `lancer()` s'occupe de la génération de l'id, de
/// l'écriture de l'index et de la sauvegarde une fois le modèle chargé.
pub fn creer_nouvelle_histoire(
    nom: &str,
    classe: &str,
    valeurs: [u32; 6],
    genre: Genre,
    premisse_libre: String,
) -> Result<EtatPartie, ErreurCreationPersonnage> {
    let personnage = creer_personnage(nom, classe, valeurs)?;
    Ok(EtatPartie {
        personnage,
        lieu_actuel: "Clairière".to_string(),
        // Pas de combat forcé dès la création : un combat ne démarre que
        // via `ActionJoueur::ProvoquerCombat` (action agressive du joueur,
        // ou décision du narrateur), jamais instantanément en même temps
        // que la scène d'ouverture.
        combat_en_cours: None,
        resumes: Vec::new(),
        pnj_connus: Vec::new(),
        // Stockée verbatim, sans troncature : la troncature se fait au
        // moment de la construction du prompt
        // (`description_premisse`/`construire_prompt_introduction`), pas
        // ici, pour rester le seul endroit qui décide de la longueur
        // affichée au modèle.
        premisse: domain::PremisseHistoire { genre, description: premisse_libre },
    })
}

/// Charge l'état d'une histoire existante depuis son fichier de
/// sauvegarde. Séparé de `lancer()` pour rester testable isolément (Review
/// Focus : fichier absent ou corrompu doit produire une erreur propre,
/// pas une panique).
fn charger_histoire_existante(chemin_sauvegarde: &Path) -> Result<EtatPartie, ErreurSauvegarde> {
    let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde.to_path_buf());
    charger_partie(&sauvegarde)
}

/// Contexte d'une histoire en cours, regroupé pour ne pas faire exploser
/// le nombre de paramètres de `traiter_message`.
pub struct ContexteHistoire<'a> {
    pub sauvegarde: &'a dyn Sauvegarde,
    pub chemin_sauvegarde: &'a Path,
    pub repertoire_sauvegardes: &'a Path,
    pub id: &'a str,
}

/// Pont entre le protocole worker<->UI et le port `GenerateurDe` : les dés
/// du JOUEUR (son jet d'attaque, son jet de dégâts) passent par ici au
/// lieu d'être tirés automatiquement — voir `JouerTour::executer`, qui les
/// distingue des dés "du monde" (ennemis, rencontre initiale), restés
/// automatiques via `infra::GenerateurDeOs` directement.
///
/// Le vrai tirage reste ici, côté worker — l'appui du joueur sur le dé
/// affiché (`MessageVersWorker::DeLancee`) ne fait que déclencher le
/// lancer, il ne choisit jamais le résultat.
///
/// L'étiquette envoyée à l'UI (`MessageVersUi::DeDemande`) est déduite du
/// nombre de faces plutôt que transmise explicitement : `GenerateurDe::lancer`
/// ne porte aucune information sur le POURQUOI du jet, et étendre sa
/// signature pour ça aurait propagé un paramètre inutile à tous ses autres
/// appelants (tests compris). Comme seul le jet d'attaque du joueur
/// utilise un d20 (l'initiative, qui utilise aussi un d20, passe par
/// `de_monde`, jamais par ici), cette règle simple reste correcte tant que
/// le jeu n'a pas d'arme ou de sort qui utiliserait aussi un d20 pour les
/// dégâts.
struct GenerateurDeJoueur<'a> {
    rx: &'a std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: &'a futures_channel::mpsc::UnboundedSender<MessageVersUi>,
}

impl GenerateurDe for GenerateurDeJoueur<'_> {
    fn lancer(&mut self, de: De, modificateur: i32) -> JetDe {
        let etiquette = if de.faces == 20 { "Jet d'attaque" } else { "Jet de dégâts" };
        let _ = self.tx.unbounded_send(MessageVersUi::DeDemande { faces: de.faces, etiquette: etiquette.to_string() });

        loop {
            match self.rx.recv() {
                Ok(MessageVersWorker::DeLancee) => break,
                // Rien d'autre n'est attendu pendant que l'UI n'affiche
                // que le dé (saisie désactivée côté UI) — on ignore tout
                // message inattendu plutôt que de planter.
                Ok(_) => continue,
                Err(_) => {
                    // Canal fermé en pleine attente d'un lancer :
                    // `GenerateurDe::lancer` ne renvoie pas de `Result`,
                    // donc impossible de remonter proprement au milieu de
                    // la résolution d'un tour — cohérent avec le reste de
                    // `lancer()`, qui traite un canal fermé comme fatal.
                    std::process::exit(0);
                }
            }
        }

        let mut de_os = infra::GenerateurDeOs;
        let jet = de_os.lancer(de, modificateur);
        let _ = self.tx.unbounded_send(MessageVersUi::DeResultat(jet.resultat));
        jet
    }
}

pub fn traiter_message(
    etat: &mut Option<EtatPartie>,
    message: MessageVersWorker,
    narrateur: &mut dyn Narrateur,
    de_joueur: &mut dyn GenerateurDe,
    de_monde: &mut dyn GenerateurDe,
    contexte: &ContexteHistoire,
    sur_message_ui: &mut dyn FnMut(MessageVersUi),
) {
    match message {
        MessageVersWorker::JouerTour(saisie) => {
            let Some(etat_courant) = etat.as_mut() else { return };
            let resultat = JouerTour::executer(etat_courant, &saisie, narrateur, de_joueur, de_monde, &mut |fragment: &str| {
                sur_message_ui(MessageVersUi::NarrationFragment(fragment.to_string()));
            });

            match resultat {
                Ok(resultat) => {
                    if resultat.partie_terminee {
                        if let Err(erreur) = std::fs::remove_file(contexte.chemin_sauvegarde) {
                            eprintln!("échec de la suppression du fichier de sauvegarde : {erreur:?}");
                        }
                        if let Err(erreur) = histoires::retirer(contexte.repertoire_sauvegardes, contexte.id) {
                            eprintln!("échec du retrait de l'histoire de l'index : {erreur:?}");
                        }
                    } else if let Err(erreur) = sauvegarder_partie(contexte.sauvegarde, etat_courant) {
                        eprintln!("échec de l'écriture de la sauvegarde : {erreur:?}");
                    }
                    sur_message_ui(MessageVersUi::TourTermine {
                        etat: etat_courant.clone(),
                        resultat_narration: resultat.narration,
                        resume_mecanique: resultat.resume_mecanique,
                        vainqueur_combat: resultat.vainqueur_combat,
                        partie_terminee: resultat.partie_terminee,
                    });
                }
                Err(erreur) => {
                    sur_message_ui(MessageVersUi::TourEchoue(format!("{:?}", erreur)));
                }
            }
        }

        // Ces messages n'atteignent jamais `traiter_message` en pratique :
        // `ReprendreHistoire`/`SupprimerHistoire`/`DemarrerCreationPersonnage`/
        // `CreerPersonnage` sont consommés par la boucle externe de
        // `lancer()` avant qu'une histoire soit chargée ; `RetourSelection`
        // par `jouer_histoire` ; `ConfirmerTelechargement` par `lancer()`
        // pendant le téléchargement ; et `DeLancee` par la boucle
        // d'attente de `GenerateurDeJoueur::lancer`. L'exhaustivité du
        // `match` impose ces bras.
        MessageVersWorker::ReprendreHistoire(_)
        | MessageVersWorker::SupprimerHistoire(_)
        | MessageVersWorker::DemarrerCreationPersonnage
        | MessageVersWorker::CreerPersonnage { .. }
        | MessageVersWorker::RetourSelection
        | MessageVersWorker::DeLancee
        | MessageVersWorker::ConfirmerTelechargement(_) => {}
    }
}

pub enum ActionApresHistoire {
    RetourSelection,
    CanalFerme,
}

/// Traite les tours d'une histoire déjà chargée jusqu'à un retour
/// explicite à la sélection (`RetourSelection`, envoyé par le bouton
/// "Retour à la sélection" de l'écran de fin) ou la fermeture du canal.
fn jouer_histoire(
    rx: &std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: &futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    etat: &mut Option<EtatPartie>,
    narrateur: &mut dyn Narrateur,
    de_monde: &mut dyn GenerateurDe,
    contexte: &ContexteHistoire,
) -> ActionApresHistoire {
    let mut de_joueur = GenerateurDeJoueur { rx, tx };
    loop {
        match rx.recv() {
            Ok(MessageVersWorker::RetourSelection) => return ActionApresHistoire::RetourSelection,
            Ok(message) => traiter_message(etat, message, narrateur, &mut de_joueur, de_monde, contexte, &mut |msg| {
                let _ = tx.unbounded_send(msg);
            }),
            Err(_) => return ActionApresHistoire::CanalFerme,
        }
    }
}

pub fn lancer(
    rx: std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    repertoire_sauvegardes: PathBuf,
    repertoire_modeles: PathBuf,
) {
    'selection: loop {
        let histoires_existantes = histoires::lister(&repertoire_sauvegardes);
        let _ = tx.unbounded_send(MessageVersUi::ListeHistoires(histoires_existantes.clone()));

        let (id, info, etat_initial, deja_enregistree) = loop {
            match rx.recv() {
                Ok(MessageVersWorker::ReprendreHistoire(id_choisie)) => {
                    let Some(info) = histoires_existantes.iter().find(|h| h.id == id_choisie).cloned() else {
                        continue; // id inconnu : on ignore, l'UI reste sur la liste déjà envoyée
                    };
                    let chemin = histoires::chemin_sauvegarde(&repertoire_sauvegardes, &id_choisie);
                    match charger_histoire_existante(&chemin) {
                        Ok(etat) => break (id_choisie, info, etat, true),
                        Err(erreur) => {
                            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                                "échec de lecture de la sauvegarde : {erreur:?}"
                            )));
                            // `continue` seul ne continuerait que la boucle
                            // interne d'attente d'une sélection, sans
                            // renvoyer `ListeHistoires` — contrairement à
                            // tous les autres chemins d'erreur de `lancer()`
                            // (taille, téléchargement, chargement du
                            // modèle), qui `continue`nt la boucle externe et
                            // renvoient donc naturellement la liste. Sans
                            // `'selection`, l'UI resterait bloquée sur
                            // l'écran d'erreur sans jamais revenir à la
                            // sélection.
                            continue 'selection;
                        }
                    }
                }
                Ok(MessageVersWorker::SupprimerHistoire(id_a_supprimer)) => {
                    if let Err(erreur) = histoires::supprimer(&repertoire_sauvegardes, &id_a_supprimer) {
                        eprintln!("échec de la suppression de l'histoire : {erreur:?}");
                    }
                    // Comme pour l'échec de lecture ci-dessus : il faut
                    // relancer la boucle EXTERNE pour que `histoires::lister`
                    // soit relu et qu'un `ListeHistoires` à jour (sans
                    // l'histoire supprimée) soit renvoyé à l'UI.
                    continue 'selection;
                }
                Ok(MessageVersWorker::DemarrerCreationPersonnage) => {
                    let mut de_tirage = infra::GenerateurDeOs;
                    let valeurs = tirer_valeurs_caracteristiques(&mut de_tirage);
                    let _ = tx.unbounded_send(MessageVersUi::ValeursProposees(valeurs));
                    continue;
                }
                Ok(MessageVersWorker::CreerPersonnage { nom, classe, valeurs, palette_modele, genre, premisse_libre }) => {
                    let Ok(etat) = creer_nouvelle_histoire(&nom, &classe, valeurs, genre, premisse_libre) else {
                        continue; // formulaire invalide : ne devrait pas arriver, la Création valide déjà
                    };
                    let nouvel_id = histoires::generer_id(&nom);
                    let info = InfoHistoire {
                        id: nouvel_id.clone(),
                        nom_personnage: nom,
                        classe,
                        palette_modele,
                        cree_le: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0),
                    };
                    break (nouvel_id, info, etat, false);
                }
                Ok(_) => continue,
                Err(_) => return,
            }
        };

        let chemin_modele = info.palette_modele.chemin_dans(&repertoire_modeles);

        if crate::telechargement::faut_il_telecharger(&chemin_modele) {
            let taille_octets = match crate::telechargement::verifier_taille_modele(&info.palette_modele) {
                Ok(taille) => taille,
                Err(erreur) => {
                    let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                        "échec de la vérification du modèle à télécharger : {erreur:?}"
                    )));
                    continue;
                }
            };

            let _ = tx.unbounded_send(MessageVersUi::ConfirmationTelechargementRequise {
                palette: info.palette_modele,
                taille_octets,
            });

            let confirme = loop {
                match rx.recv() {
                    Ok(MessageVersWorker::ConfirmerTelechargement(reponse)) => break reponse,
                    Ok(_) => continue,
                    Err(_) => return,
                }
            };

            if !confirme {
                continue; // retour à la sélection — annulation volontaire, pas une erreur
            }

            let resultat = crate::telechargement::telecharger_modele(&info.palette_modele, &chemin_modele, &mut |progression| {
                let _ = tx.unbounded_send(MessageVersUi::ProgressionTelechargement(progression));
            });
            if let Err(erreur) = resultat {
                let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                    "échec du téléchargement du modèle : {erreur:?}"
                )));
                continue;
            }
        }

        let mut narrateur = match infra::LlamaNarrateur::charger(&chemin_modele) {
            Ok(narrateur) => narrateur,
            Err(erreur) => {
                let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                    "échec du chargement du modèle : {erreur}"
                )));
                continue;
            }
        };
        let mut de = infra::GenerateurDeOs;
        let chemin_sauvegarde_histoire = histoires::chemin_sauvegarde(&repertoire_sauvegardes, &id);
        let sauvegarde = infra::SauvegardeFichier::nouveau(chemin_sauvegarde_histoire.clone());

        // Nouvelle histoire seulement : l'enregistrer maintenant que le
        // modèle a réellement chargé — pas avant, pour ne jamais laisser
        // une entrée fantôme dans l'index si le chargement échoue ou si
        // l'utilisateur annule le téléchargement.
        //
        // Ordre important : sauvegarder la partie d'abord, puis ajouter
        // l'entrée dans l'index. Si l'écriture de la sauvegarde échoue,
        // l'index ne doit pas pointer vers un fichier de sauvegarde
        // manquant.
        if !deja_enregistree {
            if let Err(erreur) = sauvegarder_partie(&sauvegarde, &etat_initial) {
                eprintln!("échec de l'écriture de la sauvegarde initiale : {erreur:?}");
            }
            if let Err(erreur) = histoires::ajouter(&repertoire_sauvegardes, info.clone()) {
                eprintln!("échec d'enregistrement de l'histoire dans l'index : {erreur:?}");
            }
        }

        let _ = tx.unbounded_send(MessageVersUi::PartieCreee(etat_initial.clone()));

        // Nouvelle histoire seulement : une scène d'ouverture plante le
        // décor avant le premier combat, plutôt que de lâcher le joueur
        // directement dans l'affrontement sans aucun contexte narratif.
        // Un échec ici n'est pas fatal — on continue sans introduction
        // plutôt que de bloquer le début de la partie pour ça.
        if !deja_enregistree {
            let resultat_intro = narrateur.raconter_introduction(
                &etat_initial.personnage,
                &etat_initial.lieu_actuel,
                &etat_initial.premisse,
                &mut |fragment: &str| {
                    let _ = tx.unbounded_send(MessageVersUi::NarrationFragment(fragment.to_string()));
                },
            );
            match resultat_intro {
                Ok(texte) => {
                    let _ = tx.unbounded_send(MessageVersUi::IntroductionRacontee(texte));
                }
                Err(erreur) => {
                    eprintln!("échec de la narration d'introduction : {erreur:?}");
                    let _ = tx.unbounded_send(MessageVersUi::IntroductionRacontee(String::new()));
                }
            }
        }

        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin_sauvegarde_histoire,
            repertoire_sauvegardes: &repertoire_sauvegardes,
            id: &id,
        };
        let mut etat = Some(etat_initial);
        match jouer_histoire(&rx, &tx, &mut etat, &mut narrateur, &mut de, &contexte) {
            ActionApresHistoire::RetourSelection => continue,
            ActionApresHistoire::CanalFerme => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::SauvegardeFichier;
    use std::path::PathBuf;

    fn chemin_temporaire(nom: &str) -> PathBuf {
        std::env::temp_dir().join(format!("app_desktop_test_{}_{}.json", std::process::id(), nom))
    }

    fn repertoire_temporaire(nom: &str) -> PathBuf {
        let chemin = std::env::temp_dir().join(format!(
            "app_desktop_worker_test_{}_{}",
            std::process::id(),
            nom
        ));
        let _ = std::fs::remove_dir_all(&chemin);
        chemin
    }

    struct GenerateurDeFixe;

    impl GenerateurDe for GenerateurDeFixe {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            domain::JetDe::nouveau(1, modificateur)
        }
    }

    #[test]
    fn creer_nouvelle_histoire_construit_un_personnage_sans_combat_force() {
        let etat = creer_nouvelle_histoire(
            "Elowen", "Guerrier", [15, 14, 13, 12, 10, 8],
            Genre::default(), String::new(),
        ).unwrap();
        assert_eq!(etat.personnage.nom, "Elowen");
        // Pas de combat forcé dès la création : un combat ne démarre que
        // via `ActionJoueur::ProvoquerCombat`, jamais instantanément en
        // même temps que la scène d'ouverture.
        assert!(etat.combat_en_cours.is_none());
    }

    #[test]
    fn creer_nouvelle_histoire_porte_le_genre_et_la_premisse_choisis() {
        let etat = creer_nouvelle_histoire(
            "Elowen", "Guerrier", [15, 14, 13, 12, 10, 8],
            Genre::Horreur, "une vieille malédiction".into(),
        ).unwrap();
        assert_eq!(etat.premisse.genre, Genre::Horreur);
        assert_eq!(etat.premisse.description, "une vieille malédiction");
    }

    /// Review Focus : reprendre une histoire dont le fichier est absent ou
    /// corrompu doit produire une erreur propre, pas une panique.
    #[test]
    fn charger_histoire_existante_echoue_proprement_si_fichier_absent() {
        let chemin = chemin_temporaire("absent");
        let _ = std::fs::remove_file(&chemin);
        assert!(charger_histoire_existante(&chemin).is_err());
    }

    #[test]
    fn charger_histoire_existante_echoue_proprement_si_json_invalide() {
        let chemin = chemin_temporaire("corrompu");
        std::fs::write(&chemin, "ceci n'est pas du json valide").unwrap();
        assert!(charger_histoire_existante(&chemin).is_err());
        let _ = std::fs::remove_file(&chemin);
    }

    struct NarrateurTourReussi;

    impl Narrateur for NarrateurTourReussi {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false })
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

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _premisse: &domain::PremisseHistoire,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
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
            premisse: Default::default(),
        }
    }

    #[test]
    fn jouer_tour_hors_combat_envoie_fragments_puis_tour_termine() {
        let chemin = chemin_temporaire("tour_reussi");
        let repertoire = repertoire_temporaire("tour_reussi");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourReussi;
        let mut de_joueur = GenerateurDeFixe;
        let mut de_monde = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de_joueur,
            &mut de_monde,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { partie_terminee: false, .. })));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurTourEnEchec;

    impl Narrateur for NarrateurTourEnEchec {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            // `Attaquer` hors combat n'échoue plus (voir `JouerTour::executer`,
            // qui retombe désormais sur `ProvoquerCombat`) : on utilise ici un
            // objet absent de l'inventaire pour continuer à exercer le chemin
            // `TourEchoue` de `traiter_message`.
            Ok(domain::ActionJoueur::UtiliserObjet { objet: "Objet inexistant".into() })
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

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _premisse: &domain::PremisseHistoire,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }
    }

    #[test]
    fn attaquer_sans_combat_envoie_tour_echoue() {
        let chemin = chemin_temporaire("tour_echec");
        let repertoire = repertoire_temporaire("tour_echec");
        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat: Option<EtatPartie> = Some(etat_sans_combat());
        let mut narrateur = NarrateurTourEnEchec;
        let mut de_joueur = GenerateurDeFixe;
        let mut de_monde = GenerateurDeFixe;
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'attaque".to_string()),
            &mut narrateur,
            &mut de_joueur,
            &mut de_monde,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourEchoue(_))));

        let _ = std::fs::remove_file(chemin);
    }

    struct NarrateurMort;

    impl Narrateur for NarrateurMort {
        fn proposer_action(
            &mut self,
            _contexte: &application::ports::narrateur::ContexteTour,
        ) -> Result<domain::ActionJoueur, application::ports::narrateur::ErreurNarrateur> {
            Ok(domain::ActionJoueur::ActionLibre { texte: "j'observe".into(), agressif: false })
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

        fn raconter_introduction(
            &mut self,
            _personnage: &domain::Personnage,
            _lieu: &str,
            _premisse: &domain::PremisseHistoire,
            _sur_fragment: &mut dyn FnMut(&str),
        ) -> Result<String, application::ports::narrateur::ErreurNarrateur> {
            Ok(String::new())
        }
    }

    /// Review Focus : la mort du personnage retire son histoire de
    /// l'index ET son fichier, sans toucher à une AUTRE histoire présente.
    #[test]
    fn mort_du_joueur_retire_son_histoire_sans_toucher_aux_autres() {
        let repertoire = repertoire_temporaire("mort");
        let chemin = histoires::chemin_sauvegarde(&repertoire, "elowen-test");
        std::fs::create_dir_all(&repertoire).unwrap();
        std::fs::write(&chemin, "contenu factice").unwrap();
        histoires::ajouter(&repertoire, InfoHistoire {
            id: "elowen-test".to_string(),
            nom_personnage: "Elowen".to_string(),
            classe: "Guerrier".to_string(),
            palette_modele: crate::modeles::PaletteModele::Equilibre,
            cree_le: 1,
        }).unwrap();
        histoires::ajouter(&repertoire, InfoHistoire {
            id: "thorn-autre".to_string(),
            nom_personnage: "Thorn".to_string(),
            classe: "Voleur".to_string(),
            palette_modele: crate::modeles::PaletteModele::Leger,
            cree_le: 2,
        }).unwrap();

        let sauvegarde = SauvegardeFichier::nouveau(chemin.clone());
        let mut etat_en_combat = etat_sans_combat();
        etat_en_combat.combat_en_cours = Some(domain::Combat {
            ennemis: vec![domain::Ennemi::nouveau("Gobelin".into(), 7, domain::De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        etat_en_combat.personnage.subir_degats(etat_en_combat.personnage.pv_max);
        let mut etat: Option<EtatPartie> = Some(etat_en_combat);
        let mut narrateur = NarrateurMort;
        // Une action libre non-agressive en combat ne lance aucun dé côté
        // joueur ; l'ennemi agit ensuite (attaque puis dégâts) via
        // `de_monde`.
        let mut de_joueur = GenerateurDeSeedLocal::nouveau(vec![]);
        let mut de_monde = GenerateurDeSeedLocal::nouveau(vec![10, 4]);
        let mut messages_envoyes = Vec::new();
        let contexte = ContexteHistoire {
            sauvegarde: &sauvegarde,
            chemin_sauvegarde: &chemin,
            repertoire_sauvegardes: &repertoire,
            id: "elowen-test",
        };

        traiter_message(
            &mut etat,
            MessageVersWorker::JouerTour("j'observe".to_string()),
            &mut narrateur,
            &mut de_joueur,
            &mut de_monde,
            &contexte,
            &mut |msg| messages_envoyes.push(msg),
        );

        assert!(matches!(messages_envoyes.last(), Some(MessageVersUi::TourTermine { partie_terminee: true, .. })));
        assert!(!chemin.exists(), "le fichier de l'histoire morte doit être supprimé");
        let histoires_restantes = histoires::lister(&repertoire);
        assert_eq!(histoires_restantes.len(), 1);
        assert_eq!(histoires_restantes[0].id, "thorn-autre", "l'autre histoire doit survivre");

        let _ = std::fs::remove_dir_all(&repertoire);
    }

    struct GenerateurDeSeedLocal {
        valeurs: std::collections::VecDeque<u32>,
    }

    impl GenerateurDeSeedLocal {
        fn nouveau(valeurs: Vec<u32>) -> Self {
            Self { valeurs: valeurs.into() }
        }
    }

    impl GenerateurDe for GenerateurDeSeedLocal {
        fn lancer(&mut self, _de: De, modificateur: i32) -> domain::JetDe {
            let brut = self.valeurs.pop_front().expect("plus de valeurs de dé");
            domain::JetDe::nouveau(brut, modificateur)
        }
    }

    /// Review Focus : deux histoires différentes choisissant le même
    /// palier doivent résoudre vers le même fichier de modèle partagé.
    #[test]
    fn deux_histoires_meme_palette_partagent_le_meme_chemin_modele() {
        use crate::modeles::PaletteModele;
        let repertoire_modeles = PathBuf::from("modeles");
        let chemin1 = PaletteModele::Equilibre.chemin_dans(&repertoire_modeles);
        let chemin2 = PaletteModele::Equilibre.chemin_dans(&repertoire_modeles);
        assert_eq!(chemin1, chemin2);
    }
}
