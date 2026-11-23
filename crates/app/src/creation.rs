use application::ports::generateur_de::GenerateurDe;
use application::usecases::creer_personnage::{creer_personnage, tirer_valeurs_caracteristiques};
use domain::{Personnage, Pronom};
use std::io::{self, Write};

pub fn creer_personnage_interactif(de: &mut dyn GenerateurDe) -> Personnage {
    println!("=== Création de personnage ===");

    let nom = lire_ligne_non_vide("Nom de ton personnage : ");
    let classe = lire_classe();
    let pronom = lire_pronom();
    let valeurs = lire_caracteristiques(de);

    creer_personnage(&nom, &classe, valeurs, pronom).expect("construction du personnage échouée")
}

// `read_line` renvoie `Ok(0)` (pas une erreur) quand l'entrée standard est
// fermée ou arrive à sa fin : sans cette vérification, ces boucles de
// relecture tournaient indéfiniment sur une saisie vide dès que l'entrée
// se terminait (ex. entrée redirigée depuis un fichier), saturant la
// sortie et la mémoire en quelques secondes.
fn quitter_si_fin_d_entree(octets_lus: usize) {
    if octets_lus == 0 {
        println!("\n(entrée terminée, arrêt.)");
        std::process::exit(0);
    }
}

fn lire_ligne_non_vide(invite: &str) -> String {
    loop {
        print!("{}", invite);
        io::stdout().flush().ok();
        let mut ligne = String::new();
        let octets_lus = io::stdin().read_line(&mut ligne).expect("lecture échouée");
        quitter_si_fin_d_entree(octets_lus);
        let ligne = ligne.trim().to_string();
        if !ligne.is_empty() {
            return ligne;
        }
        println!("La réponse ne peut pas être vide.");
    }
}

fn lire_classe() -> String {
    lire_ligne_non_vide("Classe de ton personnage (libre, ex. Guerrier, Barde, Alchimiste...) : ")
}

fn lire_pronom() -> Pronom {
    loop {
        print!("Pronom (il/elle) : ");
        io::stdout().flush().ok();
        let mut ligne = String::new();
        let octets_lus = io::stdin().read_line(&mut ligne).expect("lecture échouée");
        quitter_si_fin_d_entree(octets_lus);
        match ligne.trim().to_lowercase().as_str() {
            "il" => return Pronom::Masculin,
            "elle" => return Pronom::Feminin,
            _ => println!("Réponds par \"il\" ou \"elle\"."),
        }
    }
}

fn lire_caracteristiques(de: &mut dyn GenerateurDe) -> [u32; 6] {
    let noms = ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];
    let tirage = tirer_valeurs_caracteristiques(de);
    println!("Caractéristiques tirées pour cette aventure : {:?}", tirage);
    let mut disponibles: Vec<u32> = tirage.to_vec();
    let mut valeurs = [0u32; 6];

    for (i, nom) in noms.iter().enumerate() {
        loop {
            println!("Valeurs restantes : {:?}", disponibles);
            print!("Valeur pour {} : ", nom);
            io::stdout().flush().ok();
            let mut ligne = String::new();
            let octets_lus = io::stdin().read_line(&mut ligne).expect("lecture échouée");
            quitter_si_fin_d_entree(octets_lus);
            match ligne.trim().parse::<u32>() {
                Ok(valeur) if disponibles.contains(&valeur) => {
                    // Retire une seule occurrence, pas toutes : un tirage
                    // avec une valeur en double (`retain` les aurait
                    // toutes retirées d'un coup) laissait sinon une liste
                    // de valeurs restantes plus courte que de
                    // caractéristiques à remplir — la dernière restait
                    // bloquée sur une liste vide, impossible à satisfaire.
                    let position = disponibles.iter().position(|&v| v == valeur).unwrap();
                    disponibles.remove(position);
                    valeurs[i] = valeur;
                    break;
                }
                _ => println!("Choisis une valeur parmi celles restantes."),
            }
        }
    }

    valeurs
}
