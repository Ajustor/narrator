mod creation;

use application::ports::narrateur::Narrateur;
use application::ports::sauvegarde::ErreurSauvegarde;
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{EtatPartie, Vainqueur};
use infra::{GenerateurDeOs, LlamaNarrateur, SauvegardeFichier};
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    let sauvegarde = SauvegardeFichier::nouveau(PathBuf::from("partie.json"));
    let mut de = GenerateurDeOs;
    let chemin_modele = std::env::var("NARRATEUR_MODELE").unwrap_or_else(|_| {
        eprintln!("La variable d'environnement NARRATEUR_MODELE doit pointer vers un fichier .gguf.");
        eprintln!("Exemple : NARRATEUR_MODELE=./modeles/qwen2.5-3b-instruct-q4_k_m.gguf cargo run -p app");
        std::process::exit(1);
    });
    let mut narrateur = LlamaNarrateur::charger(std::path::Path::new(&chemin_modele)).unwrap_or_else(|erreur| {
        eprintln!("Échec du chargement du modèle : {erreur}");
        std::process::exit(1);
    });

    let mut etat: EtatPartie = match charger_partie(&sauvegarde) {
        Ok(etat) => {
            println!("Partie reprise pour {}.", etat.personnage.nom);
            etat
        }
        Err(ErreurSauvegarde::FichierAbsent) => {
            let personnage = creation::creer_personnage_interactif(&mut de);
            let mut fragment_sortie = |fragment: &str| {
                print!("{fragment}");
                io::stdout().flush().ok();
            };
            match narrateur.raconter_introduction(&personnage, "Clairière", &mut fragment_sortie) {
                Ok(_) => println!(),
                Err(erreur) => eprintln!("\n(échec de l'introduction narrée : {erreur:?})"),
            }
            // Plus de combat forcé dès la création : la première rencontre
            // se déclenche désormais au premier vrai tour, dans
            // `JouerTour::executer` (voir `rencontre_initiale_declenchee`).
            EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: None,
                resumes: Vec::new(),
                pnj_connus: Vec::new(),
                rencontre_initiale_declenchee: false,
            }
        }
        Err(ErreurSauvegarde::Lecture(message)) => {
            eprintln!("Erreur de lecture de la sauvegarde : {}", message);
            std::process::exit(1);
        }
        Err(ErreurSauvegarde::Ecriture(message)) => {
            eprintln!("Erreur d'écriture de la sauvegarde : {}", message);
            std::process::exit(1);
        }
    };

    loop {
        println!("\nQue fais-tu ?");
        print!("> ");
        io::stdout().flush().ok();
        let mut saisie = String::new();
        io::stdin().read_line(&mut saisie).expect("lecture échouée");
        let saisie = saisie.trim();

        // La console n'a pas de dé virtuel à l'écran : le joueur et le
        // monde partagent le même générateur automatique, juste séparé en
        // deux instances (sans état, `GenerateurDeOs` ne peut pas être
        // emprunté deux fois dans le même appel).
        let mut de_monde = GenerateurDeOs;
        let resultat = JouerTour::executer(&mut etat, saisie, &mut narrateur, &mut de, &mut de_monde, &mut |frag: &str| {
            print!("{}", frag);
            io::stdout().flush().ok();
        });

        match resultat {
            Ok(resultat) => {
                println!();
                for ligne in &resultat.resume_mecanique {
                    println!("{}", ligne);
                }

                match resultat.vainqueur_combat {
                    Some(Vainqueur::Joueur) => println!("\nVictoire ! Le gobelin est vaincu."),
                    Some(Vainqueur::Ennemis) => println!("\nTu es tombé au combat..."),
                    Some(Vainqueur::Fuite) => println!("\nTu as fui le combat."),
                    None => {}
                }

                if resultat.partie_terminee {
                    let _ = std::fs::remove_file("partie.json");
                    break;
                }

                sauvegarder_partie(&sauvegarde, &etat).expect("sauvegarde échouée");
            }
            Err(erreur) => {
                println!("Action invalide : {:?}", erreur);
            }
        }
    }
}
