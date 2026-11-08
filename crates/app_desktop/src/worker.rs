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

/// Répertoire de stockage privé de l'app sur Android, obtenu via JNI
/// (`Activity.getFilesDir()`) à travers les crates `ndk-context`/`jni` —
/// c'est le mécanisme standard pour ce besoin dans l'écosystème Rust/Android,
/// mais il est à vérifier contre le vrai comportement du contexte Android
/// que `dioxus`/`tao` initialisent au démarrage de l'app : si
/// `ndk_context::android_context()` panique ou si l'API `jni` résolue
/// diffère de celle utilisée ci-dessous, adapter ce code en conséquence
/// plutôt que forcer cette signature — le compilateur et le comportement
/// réel sur appareil font autorité. Partagé entre `chemin_modele_par_defaut`
/// et `chemin_sauvegarde_par_defaut` : les deux fichiers vivent dans ce même
/// répertoire, qui est le seul emplacement accessible en écriture sans
/// permission particulière sur Android (contrairement au `cwd` relatif
/// utilisé sur desktop, qui n'a pas d'équivalent garanti écrivable là-bas).
#[cfg(feature = "plateforme-android")]
fn repertoire_donnees_app() -> std::path::PathBuf {
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
    std::path::PathBuf::from(chemin_java_str.to_str().expect("UTF-8 valide"))
}

/// Chemin du fichier modèle selon la plateforme.
///
/// Desktop : inchangé, lu depuis la variable d'environnement
/// `NARRATEUR_MODELE` — ce chemin pointe déjà vers un fichier existant
/// (téléchargé par `scripts/jouer.sh`), donc `telechargement::faut_il_telecharger`
/// retournera toujours `false` en usage normal sur desktop.
///
/// Android : pas de variable d'environnement équivalente. Le modèle est
/// téléchargé dans `repertoire_donnees_app()`.
#[cfg(feature = "plateforme-desktop")]
pub fn chemin_modele_par_defaut() -> std::path::PathBuf {
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        std::process::exit(1);
    });
    std::path::PathBuf::from(chemin_modele)
}

#[cfg(feature = "plateforme-android")]
pub fn chemin_modele_par_defaut() -> std::path::PathBuf {
    repertoire_donnees_app().join("Qwen2.5-3B-Instruct-Q4_K_M.gguf")
}

/// Chemin du fichier de sauvegarde selon la plateforme.
///
/// Desktop : inchangé, `partie.json` relatif au répertoire courant du
/// processus — déjà le comportement historique de l'app, repris tel quel.
///
/// Android : un chemin relatif ne pointe vers aucun répertoire garanti
/// accessible en écriture (contrairement à desktop, où le `cwd` l'est
/// généralement) — avant ce correctif, `sauvegarder_partie` échouait
/// silencieusement à chaque tour (l'erreur est ignorée par
/// `let _ = sauvegarder_partie(...)` dans `traiter_message`), donc la
/// progression ne survivait jamais à un redémarrage de l'app. Utilise le
/// même répertoire de données privées que le modèle.
#[cfg(feature = "plateforme-desktop")]
pub fn chemin_sauvegarde_par_defaut() -> std::path::PathBuf {
    std::path::PathBuf::from("partie.json")
}

#[cfg(feature = "plateforme-android")]
pub fn chemin_sauvegarde_par_defaut() -> std::path::PathBuf {
    repertoire_donnees_app().join("partie.json")
}

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

        // Ce message n'est en réalité jamais remis à `traiter_message` : il
        // est consommé directement dans `lancer()`, avant que la boucle qui
        // appelle `traiter_message` ne démarre (voir plus bas). L'exhaustivité
        // du `match` impose néanmoins ce bras.
        MessageVersWorker::ConfirmerTelechargement(_) => {}
    }
}

pub fn charger_etat_initial(sauvegarde: &dyn Sauvegarde) -> Option<EtatPartie> {
    match charger_partie(sauvegarde) {
        Ok(etat) => Some(etat),
        Err(ErreurSauvegarde::FichierAbsent) => None,
        Err(_) => None,
    }
}

pub fn lancer(
    rx: std::sync::mpsc::Receiver<MessageVersWorker>,
    tx: futures_channel::mpsc::UnboundedSender<MessageVersUi>,
    chemin_modele: std::path::PathBuf,
    chemin_sauvegarde: std::path::PathBuf,
) {
    if crate::telechargement::faut_il_telecharger(&chemin_modele) {
        // Trace explicite : sur desktop, un `NARRATEUR_MODELE` mal
        // orthographié déclencherait sinon un téléchargement silencieux de
        // ~2 Gio au lieu de l'échec rapide d'avant, sans que rien n'indique
        // pourquoi l'application reste bloquée plusieurs minutes.
        eprintln!(
            "modèle absent à {}, vérification de sa taille avant de demander confirmation (source : {})",
            chemin_modele.display(),
            crate::telechargement::MODELE_URL
        );

        let taille_octets = match crate::telechargement::verifier_taille_modele() {
            Ok(taille) => taille,
            Err(erreur) => {
                let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                    "échec de la vérification du modèle à télécharger : {erreur:?}"
                )));
                return;
            }
        };

        let _ = tx.unbounded_send(MessageVersUi::ConfirmationTelechargementRequise(taille_octets));

        // Bloque jusqu'à la réponse de l'utilisateur. Aucun autre message ne
        // peut légitimement arriver ici : l'interface est sur l'écran de
        // confirmation, dont les deux seuls boutons envoient
        // `ConfirmerTelechargement` — on ignore donc tout le reste sans le
        // perdre (plutôt qu'un `match` qui paniquerait dessus).
        let confirme = loop {
            match rx.recv() {
                Ok(MessageVersWorker::ConfirmerTelechargement(reponse)) => break reponse,
                Ok(_) => continue,
                Err(_) => return,
            }
        };

        if !confirme {
            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(
                "téléchargement annulé — relance l'application pour réessayer".to_string(),
            ));
            return;
        }

        let resultat = crate::telechargement::telecharger_modele(&chemin_modele, &mut |progression| {
            let _ = tx.unbounded_send(MessageVersUi::ProgressionTelechargement(progression));
        });
        if let Err(erreur) = resultat {
            let _ = tx.unbounded_send(MessageVersUi::TourEchoue(format!(
                "échec du téléchargement du modèle : {erreur:?}"
            )));
            return;
        }
    }

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
