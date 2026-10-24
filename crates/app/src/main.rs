mod creation;

use application::usecases::commencer_combat::commencer_combat;
use application::usecases::jouer_tour::JouerTour;
use application::usecases::partie::{charger_partie, sauvegarder_partie};
use domain::{De, Ennemi, EtatPartie, Vainqueur};
use infra::{FauxNarrateur, GenerateurDeOs, SauvegardeFichier};
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    let sauvegarde = SauvegardeFichier::nouveau(PathBuf::from("partie.json"));
    let mut de = GenerateurDeOs;
    let mut narrateur = FauxNarrateur;

    let mut etat: EtatPartie = match charger_partie(&sauvegarde) {
        Ok(etat) => {
            println!("Partie reprise pour {}.", etat.personnage.nom);
            etat
        }
        Err(_) => {
            let personnage = creation::creer_personnage_interactif();
            let gobelin = Ennemi::nouveau("Gobelin".to_string(), 7, De { faces: 6 }, 12, 2, 3);
            let combat = commencer_combat(&personnage, vec![gobelin], &mut de);
            EtatPartie {
                personnage,
                lieu_actuel: "Clairière".to_string(),
                combat_en_cours: Some(combat),
                resumes: Vec::new(),
            }
        }
    };

    loop {
        println!("\nQue fais-tu ?");
        print!("> ");
        io::stdout().flush().ok();
        let mut saisie = String::new();
        io::stdin().read_line(&mut saisie).expect("lecture échouée");
        let saisie = saisie.trim();

        let resultat = JouerTour::executer(&mut etat, saisie, &mut narrateur, &mut de, &mut |frag: &str| {
            print!("{}", frag);
            io::stdout().flush().ok();
        });

        match resultat {
            Ok(resultat) => {
                println!();
                for ligne in &resultat.resume_mecanique {
                    println!("{}", ligne);
                }
                sauvegarder_partie(&sauvegarde, &etat).expect("sauvegarde échouée");

                if resultat.combat_termine {
                    match etat.combat_en_cours.as_ref().and_then(|c| c.vainqueur) {
                        Some(Vainqueur::Joueur) => println!("\nVictoire ! Le gobelin est vaincu."),
                        Some(Vainqueur::Ennemis) => println!("\nTu es tombé au combat..."),
                        Some(Vainqueur::Fuite) => println!("\nTu as fui le combat."),
                        None => {}
                    }
                    break;
                }
            }
            Err(erreur) => {
                println!("Action invalide : {:?}", erreur);
            }
        }
    }
}
